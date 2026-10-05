// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::{expand, expand_or_compile_error, inner_type};
use quote::quote;
use syn::Type;

#[test]
fn rejects_unsupported_signatures() {
    for source in [
        "async fn example() {}",
        "unsafe fn example() {}",
        "extern \"C\" fn example() {}",
        "fn example<T>() {}",
        "fn example() where (): Copy {}",
        "fn example(&self) {}",
        "fn example((first, second): (String, String)) {}",
        "fn example(value: &str) {}",
        "fn example(ref value: String) {}",
        "fn example(value @ _: String) {}",
        "fn example(#[bake(default = 1)] value: Option<usize>) {}",
        "fn example(#[bake(default = 1)] value: Vec<usize>) {}",
        "fn example(context: &mut Context, #[bake(context)] other: &mut Context) {}",
        "fn example(#[bake(unknown)] value: String) {}",
        "fn example(#[bake(default = 1, default = 2)] value: usize) {}",
        "fn example(#[bake(default)] value: usize) {}",
        "fn example(#[bake(default = let)] value: usize) {}",
        "fn example(#[bake(help)] value: String) {}",
        "fn example(#[bake(help = 42)] value: String) {}",
        "fn example(#[bake(input)] value: String) {}",
        "fn example(#[bake(input)] first: Value, #[bake(input)] second: Value) {}",
        "fn example(#[bake(input, named)] value: Value) {}",
        "fn example(#[bake(positional)] value: String) {}",
        "fn example(#[bake(named, positional)] values: Vec<String>) {}",
        "fn example(#[bake(positional)] values: Vec<String>, name: String) {}",
        "fn example(#[bake(positional)] first: Vec<String>, #[bake(positional)] second: Vec<String>) {}",
        "fn example(#[bake(input, positional)] value: Value) {}",
        "fn example(#[bake(context, positional)] context: &mut Context) {}",
    ] {
        assert!(
            expand(quote!(), syn::parse_str(source).unwrap()).is_err(),
            "{source}"
        );
    }
}

#[test]
fn renamed_runtime_and_custom_task_name_expand() {
    let output = expand(
        quote!(name = "nested:example", runtime = ::renamed),
        syn::parse_quote! {
            /// Some helpful text.
            pub fn example(#[bake(default = "value")] name: String) -> Result<String> { Ok(name) }
        },
    )
    .unwrap()
    .to_string();
    assert!(output.contains("nested:example"));
    assert!(output.contains("renamed :: Task"));
    assert!(output.contains("Some helpful text."));
    assert!(output.contains("pub fn example_task"));
}

#[test]
fn rejects_unknown_task_options() {
    assert!(
        expand(
            quote!(unknown),
            syn::parse_quote!(
                fn example() {}
            )
        )
        .is_err()
    );
}

#[test]
fn reports_invalid_values_for_task_options() {
    let function: syn::ItemFn = syn::parse_quote!(
        fn example() {}
    );

    assert!(expand(quote!(name), function.clone()).is_err());
    assert!(expand(quote!(name = 42), function.clone()).is_err());
    assert!(expand(quote!(runtime), function.clone()).is_err());
    assert!(expand(quote!(runtime = 42), function).is_err());
}

#[test]
fn compile_error_adapter_preserves_successful_expansions() {
    let output = expand_or_compile_error(
        quote!(),
        syn::parse_quote!(
            fn example() -> Result<()> {
                Ok(())
            }
        ),
    )
    .to_string();

    assert!(output.contains("example_task"));
    assert!(!output.contains("compile_error"));
}

#[test]
fn converts_expansion_errors_to_compile_errors() {
    let output = expand_or_compile_error(
        quote!(unknown),
        syn::parse_quote!(
            fn example() {}
        ),
    )
    .to_string();

    assert!(output.contains("compile_error"));
    assert!(output.contains("expected name, runtime, output, or builtin"));
}

#[test]
fn parameter_options_preserve_other_attributes() {
    let output = expand(
        quote!(),
        syn::parse_quote! {
            fn example(#[allow(unused_variables)] value: String) {}
        },
    )
    .unwrap()
    .to_string();

    assert!(output.contains("allow (unused_variables)"));
}

#[test]
fn inner_type_rejects_unexpected_path_arguments() {
    let non_path: Type = syn::parse_quote!(&str);
    let empty_path = Type::Path(syn::TypePath {
        qself: None,
        path: syn::Path {
            leading_colon: None,
            segments: syn::punctuated::Punctuated::new(),
        },
    });
    let no_arguments: Type = syn::parse_quote!(Option);
    let multiple_arguments: Type = syn::parse_quote!(std::collections::HashMap<String, usize>);
    let non_type_argument: Type = syn::parse_quote!(Option<'static>);

    assert!(inner_type("Option", &non_path).is_none());
    assert!(inner_type("Option", &empty_path).is_none());
    assert!(inner_type("Option", &no_arguments).is_none());
    assert!(inner_type("HashMap", &multiple_arguments).is_none());
    assert!(inner_type("Option", &non_type_argument).is_none());
}

#[test]
fn output_and_previous_input_options_expand() {
    let output = expand(
        quote!(output),
        syn::parse_quote! {
            fn render(#[bake(input)] input: Value) -> Result<Value> { Ok(input) }
        },
    )
    .unwrap()
    .to_string();
    assert!(output.contains("handles_output"));
    assert!(output.contains("TASK_REGISTRATIONS"));
    assert!(output.contains("previous"));
    assert!(!output.contains("Parameter :: new"));
    assert!(
        expand(
            quote!(output, output),
            syn::parse_quote!(
                fn example() {}
            )
        )
        .is_err()
    );
}

#[test]
fn expands_optional_repeated_context_and_named_parameters() {
    let output = expand(
        quote!(builtin),
        syn::parse_quote! {
            #[cfg(unix)]
            /// Describe the generated task.
            fn example(
                #[bake(help = "An optional value.")] optional: Option<String>,
                #[bake(named, help = "Repeat this value.")] values: Vec<String>,
                #[bake(default = 7, named, help = "A numeric default.")] count: usize,
                #[bake(context)] context: &mut Context,
            ) -> Result<usize> {
                let _ = (optional, values, context);
                Ok(count)
            }
        },
    )
    .unwrap()
    .to_string();

    assert!(output.contains("cfg (unix)"));
    assert!(output.contains("optional ()"));
    assert!(output.contains("repeated ()"));
    assert!(output.contains("default (\"7\")"));
    assert!(output.contains("help (\"A numeric default.\")"));
    assert!(output.contains("builtin : true"));
    assert!(output.contains("Describe the generated task."));
}

#[test]
fn expands_positional_variadic_arguments() {
    let output = expand(
        quote!(),
        syn::parse_quote! {
            fn example(#[bake(positional, help = "Input files.")] paths: Vec<String>) -> Result<Vec<String>> {
                Ok(paths)
            }
        },
    )
    .unwrap()
    .to_string();

    assert!(output.contains("variadic ()"));
    assert!(output.contains("repeated :: < String >"));
    assert!(output.contains("Input files."));
}

#[test]
fn expands_context_parameters_by_convention() {
    let output = expand(
        quote!(),
        syn::parse_quote! {
            fn example(context: &mut Context) -> Result<()> {
                let _ = context;
                Ok(())
            }
        },
    )
    .unwrap()
    .to_string();

    assert!(output.contains("Task :: new"));
    assert!(!output.contains("Parameter :: new"));
}
