// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Compile ordinary Rust functions into explicit Bake task descriptors.
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Expr, FnArg, ItemFn, LitStr, Pat, Path, Type, parse_macro_input};

/// Preserve a synchronous function and generate `function_task()` beside it.
/// Task functions return a Result whose successful value implements Serialize.
#[proc_macro_attribute]
pub fn task(attributes: TokenStream, input: TokenStream) -> TokenStream {
    let function = parse_macro_input!(input as ItemFn);
    expand_or_compile_error(attributes.into(), function).into()
}

fn expand_or_compile_error(
    attributes: proc_macro2::TokenStream,
    function: ItemFn,
) -> proc_macro2::TokenStream {
    match expand(attributes, function) {
        Ok(output) => output,
        Err(error) => error.into_compile_error(),
    }
}

#[derive(Default)]
struct ParameterOptions {
    default: Option<Expr>,
    named: bool,
    context: bool,
    input: bool,
    help: Option<LitStr>,
}

fn options(attributes: &mut Vec<Attribute>) -> syn::Result<ParameterOptions> {
    let mut options = ParameterOptions::default();
    let mut retained = Vec::new();
    for attribute in attributes.drain(..) {
        if !attribute.path().is_ident("bake") {
            retained.push(attribute);
            continue;
        }
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("default") {
                if options.default.is_some() {
                    return Err(meta.error("duplicate default"));
                }
                options.default = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("named") {
                options.named = true;
            } else if meta.path.is_ident("context") {
                options.context = true;
            } else if meta.path.is_ident("input") {
                options.input = true;
            } else if meta.path.is_ident("help") {
                options.help = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error("expected default, named, context, input, or help"));
            }
            Ok(())
        })?;
    }
    *attributes = retained;
    Ok(options)
}

fn inner_type<'a>(kind: &str, value: &'a Type) -> Option<&'a Type> {
    let Type::Path(path) = value else { return None };
    let segment = path.path.segments.last()?;
    if segment.ident != kind {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    if arguments.args.len() != 1 {
        return None;
    }
    match arguments.args.first()? {
        syn::GenericArgument::Type(value) => Some(value),
        _ => None,
    }
}

fn expand(
    attributes: proc_macro2::TokenStream,
    mut function: ItemFn,
) -> syn::Result<proc_macro2::TokenStream> {
    use syn::parse::Parser;
    let mut name = None::<LitStr>;
    let mut handles_output = false;
    let mut output_option_seen = false;
    let mut builtin = false;
    let mut runtime: Path = syn::parse_quote!(::bake);
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("name") {
            name = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("runtime") {
            runtime = meta.value()?.parse()?;
        } else if meta.path.is_ident("output") {
            if output_option_seen {
                return Err(meta.error("duplicate output option"));
            }
            output_option_seen = true;
            handles_output = true;
        } else if meta.path.is_ident("builtin") {
            builtin = true;
        } else {
            return Err(meta.error("expected name, runtime, output, or builtin"));
        }
        Ok(())
    });
    parser.parse2(attributes)?;
    if function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || function.sig.variadic.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
    {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "tasks must be safe, synchronous, non-generic Rust functions",
        ));
    }
    let function_name = &function.sig.ident;
    let descriptor_name = format_ident!("{}_task", function_name);
    let registration_name = format_ident!(
        "__BAKE_REGISTER_{}",
        function_name
            .to_string()
            .trim_start_matches("r#")
            .to_uppercase()
    );
    let infer_crate_namespace = name.is_none();
    let command_name = name.unwrap_or_else(|| {
        LitStr::new(
            function_name.to_string().trim_start_matches("r#"),
            function_name.span(),
        )
    });
    let visibility = &function.vis;
    let mut documentation = Vec::new();
    for attribute in &function.attrs {
        if attribute.path().is_ident("doc")
            && let syn::Meta::NameValue(value) = &attribute.meta
            && let Expr::Lit(value) = &value.value
            && let syn::Lit::Str(value) = &value.lit
        {
            documentation.push(value.value().trim().to_owned());
        }
    }
    let documentation = documentation.join("\n");
    let conditional_attributes: Vec<_> = function
        .attrs
        .iter()
        .filter(|attribute| {
            attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
        })
        .cloned()
        .collect();
    let mut parameters = Vec::new();
    let mut bindings = Vec::new();
    let mut call_arguments = Vec::new();
    let invocation_context =
        format_ident!("__bake_context", span = proc_macro2::Span::mixed_site());
    let invocation_arguments =
        format_ident!("__bake_arguments", span = proc_macro2::Span::mixed_site());
    let mut has_context = false;
    let mut has_input = false;
    for input in &mut function.sig.inputs {
        let FnArg::Typed(argument) = input else {
            return Err(syn::Error::new_spanned(
                input,
                "task methods with self are not supported",
            ));
        };
        let Pat::Ident(pattern) = argument.pat.as_ref() else {
            return Err(syn::Error::new_spanned(
                &argument.pat,
                "task parameters must have simple names",
            ));
        };
        if pattern.by_ref.is_some() || pattern.subpat.is_some() {
            return Err(syn::Error::new_spanned(
                pattern,
                "task parameters must have simple names",
            ));
        }
        let identifier = &pattern.ident;
        let parameter_name = identifier.to_string().trim_start_matches("r#").to_owned();
        let settings = options(&mut argument.attrs)?;
        let parameter_type = argument.ty.as_ref();
        if settings.input {
            let is_value = matches!(parameter_type, Type::Path(path)
                if path.path.segments.last().is_some_and(|segment| {
                    segment.ident == "Value" && matches!(segment.arguments, syn::PathArguments::None)
                })
            );
            if !is_value
                || has_input
                || settings.default.is_some()
                || settings.named
                || settings.context
                || settings.help.is_some()
            {
                return Err(syn::Error::new_spanned(
                    argument,
                    "use one #[bake(input)] owned bake::Value parameter without other options",
                ));
            }
            has_input = true;
            bindings.push(
                quote!(let #identifier: #parameter_type = #invocation_context.previous().clone();),
            );
            call_arguments.push(quote!(#identifier));
            continue;
        }
        let is_context = settings.context
            || (parameter_name == "context" && matches!(parameter_type, Type::Reference(_)));
        if is_context {
            if has_context
                || settings.default.is_some()
                || settings.named
                || settings.help.is_some()
                || settings.input
            {
                return Err(syn::Error::new_spanned(
                    argument,
                    "use one context parameter without argument options",
                ));
            }
            has_context = true;
            call_arguments.push(quote!(#invocation_context));
            continue;
        }
        if matches!(parameter_type, Type::Reference(_)) {
            return Err(syn::Error::new_spanned(
                parameter_type,
                "use owned task arguments such as String or PathBuf",
            ));
        }
        let optional_type = inner_type("Option", parameter_type);
        let repeated_type = inner_type("Vec", parameter_type);
        if settings.default.is_some() && (optional_type.is_some() || repeated_type.is_some()) {
            return Err(syn::Error::new_spanned(
                parameter_type,
                "Option and Vec already default to None and empty; use a scalar for an explicit default",
            ));
        }
        let parsed_type = optional_type.or(repeated_type).unwrap_or(parameter_type);
        let mut parameter = quote!(#runtime::Parameter::new::<#parsed_type>(#parameter_name));
        let binding = if optional_type.is_some() {
            parameter = quote!(#parameter.named().optional());
            quote!(#invocation_arguments.optional::<#parsed_type>(#parameter_name)?)
        } else if repeated_type.is_some() {
            parameter = quote!(#parameter.repeated());
            quote!(#invocation_arguments.repeated::<#parsed_type>(#parameter_name)?)
        } else if let Some(default) = &settings.default {
            let description = quote!(#default).to_string();
            parameter = quote!(#parameter.default(#description));
            // String literals conveniently initialize String/PathBuf/custom types.
            // Other expressions retain the parameter's type inference (e.g. 1
            // for a usize) and must produce that parameter's type.
            let default = if matches!(default, Expr::Lit(value) if matches!(value.lit, syn::Lit::Str(_)))
            {
                quote!((#default).into())
            } else {
                quote!(#default)
            };
            quote!(#invocation_arguments.optional::<#parsed_type>(#parameter_name)?.unwrap_or_else(|| #default))
        } else {
            quote!(#invocation_arguments.required::<#parsed_type>(#parameter_name)?)
        };
        if settings.named {
            parameter = quote!(#parameter.named());
        }
        if let Some(help) = settings.help {
            parameter = quote!(#parameter.help(#help));
        }
        parameters.push(parameter);
        bindings.push(quote!(let #identifier: #parameter_type = #binding;));
        call_arguments.push(quote!(#identifier));
    }
    let task = quote! {
            #runtime::Task::new(#command_name, #documentation, vec![#(#parameters),*], |#invocation_context, #invocation_arguments| {
                #(#bindings)*
                let output = #function_name(#(#call_arguments),*).map_err(|error| #runtime::Error::new(error.to_string()))?;
                #runtime::value(output)
            })
    };
    let task = if handles_output {
        quote!(#task.handles_output())
    } else {
        task
    };
    let builtin = syn::LitBool::new(builtin, proc_macro2::Span::call_site());
    Ok(quote! {
        #function

        #(#conditional_attributes)*
        #[doc = "Generated Bake task descriptor."]
        #visibility fn #descriptor_name() -> #runtime::Task {
            #task
        }

        #(#conditional_attributes)*
        #[doc(hidden)]
        #[#runtime::__private::linkme::distributed_slice(#runtime::__private::TASK_REGISTRATIONS)]
        #[linkme(crate = #runtime::__private::linkme)]
        static #registration_name: #runtime::__private::TaskRegistration =
            #runtime::__private::TaskRegistration {
                factory: #descriptor_name,
                module_path: module_path!(),
                infer_crate_namespace: #infer_crate_namespace && option_env!("CARGO_BIN_NAME").is_none(),
                builtin: #builtin,
            };
    })
}

#[cfg(test)]
mod expansion_tests;
