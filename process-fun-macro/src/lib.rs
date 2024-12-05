use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, Error, ItemFn, Visibility};

#[proc_macro_attribute]
pub fn process(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    
    // Check for duplicate process attributes
    let process_attrs: Vec<_> = input_fn.attrs.iter()
        .filter(|attr| attr.path().is_ident("process"))
        .collect();
    
    if process_attrs.len() > 1 {
        panic!("#[process] can only be used once per function");
    }

    // Ensure the function is public
    match input_fn.vis {
        Visibility::Public(_) => {},
        _ => panic!("#[process] can only be used on public functions"),
    }

    let fn_name = &input_fn.sig.ident;
    let process_fn_name = format_ident!("{}_process", fn_name);
    let hash_static_name = format_ident!("{}_PROCESS_HASH", fn_name.to_string().to_uppercase());
    
    let fn_args = &input_fn.sig.inputs;
    let fn_output = match &input_fn.sig.output {
        syn::ReturnType::Default => quote!(()),
        syn::ReturnType::Type(_, ty) => quote!(#ty),
    };
    
    let arg_names: Vec<_> = fn_args.iter().map(|arg| {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    &pat_ident.ident
                } else {
                    panic!("Unsupported argument pattern")
                }
            }
            syn::FnArg::Receiver(_) => {
                panic!("Self arguments not supported")
            }
        }
    }).collect();

    let arg_types: Vec<_> = fn_args.iter().map(|arg| {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                pat_type.ty.clone()
            }
            syn::FnArg::Receiver(_) => {
                panic!("Self arguments not supported")
            }
        }
    }).collect();

    let args_tuple = quote! { (#(#arg_names),*) };
    let args_types_tuple = quote! { (#(#arg_types),*) };
    let fn_name_str = fn_name.to_string();

    let expanded = quote! {
        use process_fun_core::ProcessFunError;

        #input_fn

        static #hash_static_name: process_fun_core::once_cell::sync::Lazy<String> = 
            process_fun_core::once_cell::sync::Lazy::new(|| {
                process_fun_core::generate_unique_hash(
                    #fn_name_str,
                    &quote::quote!(#args_tuple).to_string(),
                    &quote::quote!(#fn_output).to_string()
                )
            });

        #[allow(non_snake_case)]
        pub fn #process_fn_name(#fn_args) -> Result<#fn_output, ProcessFunError> {
            use std::process::Command;
            use serde_json;
            
            // Serialize arguments to JSON
            let args_tuple = #args_tuple;
            let args_json = serde_json::to_string(&args_tuple)?;
            
            // Get current executable path
            let current_exe = std::env::current_exe()?;
            
            // Spawn process with arguments
            let output = Command::new(current_exe)
                .arg(process_fun_core::generate_unique_hash(
                    #fn_name_str,
                    &quote::quote!(#args_tuple).to_string(),
                    &quote::quote!(#fn_output).to_string()
                ))
                .arg(args_json)
                .output()?;

            if !output.status.success() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    String::from_utf8_lossy(&output.stderr)
                ).into());
            }

            // Deserialize result
            let result: #fn_output = serde_json::from_slice(&output.stdout)?;
            Ok(result)
        }

        inventory::submit! {
            process_fun_core::ProcessFunction {
                name: #fn_name_str,
                hash: &#hash_static_name,
                handler: |args_json: String| -> Option<String> {
                    let args: #args_types_tuple = serde_json::from_str(&args_json).ok()?;
                    let (#(#arg_names),*) = args;
                    let result = #fn_name(#(#arg_names),*);
                    serde_json::to_string(&result).ok()
                }
            }
        }
    };

    // Convert to proc_macro::TokenStream
    TokenStream::from(expanded)
}
