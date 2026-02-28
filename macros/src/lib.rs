use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};

/// Skip a test when the named environment variable is not set.
///
/// ```ignore
/// #[test]
/// #[require_env("PICO_IP")]
/// fn my_hardware_test() {
///     // only runs when PICO_IP is set
/// }
/// ```
#[proc_macro_attribute]
pub fn require_env(attr: TokenStream, item: TokenStream) -> TokenStream {
    let env_var = parse_macro_input!(attr as LitStr);
    let mut func = parse_macro_input!(item as ItemFn);

    let var_name = env_var.value();
    let fn_name = func.sig.ident.to_string();
    let original_body = &func.block;

    let new_body = syn::parse_quote!({
        if ::std::env::var(#var_name).is_err() {
            ::std::eprintln!("{} not set — skipping {}", #var_name, #fn_name);
            return;
        }
        #original_body
    });

    func.block = Box::new(new_body);

    TokenStream::from(quote! { #func })
}
