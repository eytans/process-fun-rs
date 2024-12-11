//! # process-fun-macro
//! 
//! Procedural macros for the process-fun library. This crate provides the implementation
//! of the `#[process]` attribute macro.
//! 
//! This crate is not meant to be used directly - instead, use the `process-fun` crate
//! which re-exports these macros in a more convenient way.

use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, Error, ItemFn, Visibility};

/// Attribute macro that creates an additional version of a function that executes in a separate process.
/// 
/// When applied to a function named `foo`, this macro:
/// 1. Keeps the original function unchanged, allowing normal in-process calls
/// 2. Creates a new function named `foo_process` that executes in a separate process
/// 
/// # Requirements
/// 
/// The function must:
/// * Be public (`pub`)
/// * Have arguments and return type that implement `Serialize` and `Deserialize`
/// * Not take `self` parameters
/// 
/// # Generated Code
/// 
/// For a function named `foo`, this macro generates:
/// * A static hash value unique to the function
/// * A wrapper function named `foo_process` that handles out-of-process execution
/// * Registration code that allows the function to be called from child processes
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

    let debug_prints = quote! {
        #[cfg(feature = "debug")]
        {
            eprintln!("[process-fun-debug] Processing function: {}", #fn_name_str);
            eprintln!("[process-fun-debug] Generated hash: {}", *#hash_static_name);
            eprintln!("[process-fun-debug] Arguments tuple type: {}", stringify!(#args_types_tuple));
            eprintln!("[process-fun-debug] Serialized arguments: {}", args_json);
            eprintln!("[process-fun-debug] Current executable: {:?}", current_exe);
        }
    };

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
            
            // Create pipes for communication
            let (arg_read, arg_write, mut result_read, result_write) = process_fun_core::create_pipes()?;
            
            // Serialize arguments to JSON
            let args_tuple = #args_tuple;
            let args_json = serde_json::to_string(&args_tuple)?;
            
            // Get current executable path
            let current_exe = std::env::current_exe()?;

            #debug_prints
            
            let hashp = process_fun_core::generate_unique_hash(
                #fn_name_str,
                &quote::quote!(#args_tuple).to_string(),
                &quote::quote!(#fn_output).to_string()
            );

            #[cfg(feature = "debug")]
            eprintln!("[process-fun-debug] Generated hash for process: {}", hashp);

            // Write arguments to pipe before spawning child
            process_fun_core::write_to_pipe(arg_write, args_json.as_bytes())?;

            // Get handles in a platform-independent way
            let arg_handle = process_fun_core::get_pipe_handle(&arg_read);
            let result_handle = process_fun_core::get_pipe_handle(&result_write);

            // Spawn process with handles
            let mut child = Command::new(current_exe);
            child
                .env(process_fun_core::ENV_FUNCTION_HASH, &hashp)
                .env(process_fun_core::ENV_ARG_FD, arg_handle)
                .env(process_fun_core::ENV_RESULT_FD, result_handle);

            #[cfg(feature = "test-debug")] 
            {
                child.arg("--nocapture");
            }
            
            let mut child = child.spawn()?;

            // Drop the handles that were passed to the child
            drop(arg_read);
            drop(result_write);

            #[cfg(feature = "debug")]
            eprintln!("[process-fun-debug] Child process spawned. Waiting for completion...");

            // Wait for child process to complete
            let status = child.wait()?;

            if !status.success() {
                return Err(ProcessFunError::ProcessError("Child process failed".to_string()));
            }

            #[cfg(feature = "debug")]
            eprintln!("[process-fun-debug] Child process completed. Reading from pipe...");

            // Read result from pipe
            let result_bytes = process_fun_core::read_from_pipe(&mut result_read)?;
            
            // Drop the read pipe after getting result
            drop(result_read);

            let result: #fn_output = serde_json::from_slice(&result_bytes)?;

            #[cfg(feature = "debug")]
            {
                eprintln!("[process-fun-debug] Deserialized result: {:?}", result);
            }

            Ok(result)
        }

        inventory::submit! {
            process_fun_core::ProcessFunction {
                name: #fn_name_str,
                hash: &#hash_static_name,
                handler: |args_json: String| -> Option<String> {
                    #[cfg(feature = "debug")]
                    {
                        eprintln!("[process-fun-debug] Handler called for function: {}", #fn_name_str);
                        eprintln!("[process-fun-debug] Received args_json: {}", args_json);
                    }

                    let args: #args_types_tuple = serde_json::from_str(&args_json).ok()?;
                    let (#(#arg_names),*) = args;
                    let result = #fn_name(#(#arg_names),*);

                    #[cfg(feature = "debug")]
                    {
                        eprintln!("[process-fun-debug] Handler result: {:?}", result);
                    }

                    serde_json::to_string(&result).ok()
                }
            }
        }
    };

    #[cfg(feature = "debug")]
    {
        dbg!(expanded.to_string());
    }

    // Convert to proc_macro::TokenStream
    TokenStream::from(expanded)
}
