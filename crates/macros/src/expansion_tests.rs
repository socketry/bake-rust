use super::expand;
use quote::quote;

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
        "fn example(#[bake(default = 1)] value: Option<usize>) {}",
        "fn example(#[bake(default = 1)] value: Vec<usize>) {}",
        "fn example(context: &mut Context, #[bake(context)] other: &mut Context) {}",
        "fn example(#[bake(unknown)] value: String) {}",
        "fn example(#[bake(default = 1, default = 2)] value: usize) {}",
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
