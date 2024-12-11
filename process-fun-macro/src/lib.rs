//! # process-fun-macro
//!
//! Procedural macros for the process-fun library. This crate provides the implementation
//! of the `#[process]` attribute macro.
//!
//! This crate is not meant to be used directly - instead, use the `process-fun` crate
//! which re-exports these macros in a more convenient way.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, ItemFn, Visibility};

/// Attribute macro that creates an additional version of a function that executes in a separate process.
///
/// When applied to a function named `foo`, this macro:
/// 1. Keeps the original function unchanged, allowing normal in-process calls
/// 2. Creates a new function named `foo_process` that executes in a separate process using fork
///
/// # Requirements
///
/// The function must:
/// * Be public (`pub`)
/// * Have arguments and return type that implement `Serialize` and `Deserialize`
/// * Not take `self` parameters
///
/// # Example
///
/// ```rust
/// use process_fun::process;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Point {
///     x: i32,
///     y: i32,
/// }
///
/// #[process]
/// pub fn add_points(p1: Point, p2: Point) -> Point {
///     Point {
///         x: p1.x + p2.x,
///         y: p1.y + p2.y,
///     }
/// }
///
/// // Now you can use either:
/// // add_points(p1, p2)           // runs in the current process
/// // add_points_process(p1, p2)   // runs in a separate process
/// ```
#[proc_macro_attribute]
pub fn process(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    // Check for duplicate process attributes
    let process_attrs: Vec<_> = input_fn
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("process"))
        .collect();

    if process_attrs.len() > 1 {
        panic!("#[process] can only be used once per function");
    }

    // Ensure the function is public
    match input_fn.vis {
        Visibility::Public(_) => {}
        _ => panic!("#[process] can only be used on public functions"),
    }

    let fn_name = &input_fn.sig.ident;
    let process_fn_name = format_ident!("{}_process", fn_name);

    let fn_args = &input_fn.sig.inputs;
    let fn_output = match &input_fn.sig.output {
        syn::ReturnType::Default => quote!(()),
        syn::ReturnType::Type(_, ty) => quote!(#ty),
    };

    let arg_names: Vec<_> = fn_args
        .iter()
        .map(|arg| match arg {
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
        })
        .collect();

    let arg_types: Vec<_> = fn_args
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => pat_type.ty.clone(),
            syn::FnArg::Receiver(_) => {
                panic!("Self arguments not supported")
            }
        })
        .collect();

    let args_tuple = quote! { (#(#arg_names),*) };
    let args_types_tuple = quote! { (#(#arg_types),*) };
    let fn_name_str = fn_name.to_string();

    let debug_prints = quote! {
        #[cfg(feature = "debug")]
        {
            eprintln!("[process-fun-debug] Processing function: {}", #fn_name_str);
            eprintln!("[process-fun-debug] Arguments tuple type: {}", stringify!(#args_types_tuple));
            eprintln!("[process-fun-debug] Arguments: {:?}", #args_tuple);
        }
    };

    let expanded = quote! {
        use process_fun_core::ProcessFunError;
        use nix::unistd::ForkResult;

        #input_fn

        #[allow(non_snake_case)]
        pub fn #process_fn_name(#fn_args) -> Result<#fn_output, ProcessFunError> {
            use serde_json;

            // Create pipe for result communication
            let (mut read_pipe, write_pipe) = process_fun_core::create_pipes()?;

            // Fork the process
            match process_fun_core::fork_process()? {
                ForkResult::Parent { .. } => {
                    // Parent process
                    drop(write_pipe);

                    #[cfg(feature = "debug")]
                    eprintln!("[process-fun-debug] Parent process waiting for result...");

                    // Read result from pipe
                    let result_bytes = process_fun_core::read_from_pipe(&mut read_pipe)?;
                    drop(read_pipe);

                    let result: #fn_output = serde_json::from_slice(&result_bytes)?;

                    #[cfg(feature = "debug")]
                    eprintln!("[process-fun-debug] Parent received result: {:?}", result);

                    Ok(result)
                }
                ForkResult::Child => {
                    // Child process
                    drop(read_pipe);

                    #[cfg(feature = "debug")]
                    eprintln!("[process-fun-debug] Child process started");

                    #debug_prints

                    // Execute the function with the original arguments
                    let result = #fn_name(#(#arg_names),*);

                    // Write result back to parent
                    let result_json = serde_json::to_string(&result)?;

                    #[cfg(feature = "debug")]
                    eprintln!("[process-fun-debug] Child process result: {:?}", result);

                    process_fun_core::write_to_pipe(write_pipe, result_json.as_bytes())?;

                    #[cfg(feature = "debug")]
                    eprintln!("[process-fun-debug] Child process completed");

                    // Exit child process
                    std::process::exit(0);
                }
            }
        }
    };

    #[cfg(feature = "debug")]
    {
        dbg!(expanded.to_string());
    }

    TokenStream::from(expanded)
}
