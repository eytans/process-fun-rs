//! # process-fun-macro
//!
//! Procedural macros for the process-fun library. This crate provides the implementation
//! of the `#[process]` attribute macro.
//!
//! This crate is not meant to be used directly - instead, use the `process-fun` crate
//! which re-exports these macros in a more convenient way.

use proc_macro::TokenStream;
use proc_macro_error::{proc_macro_error, Diagnostic, Level};
use quote::{format_ident, quote};
use syn::{parse_macro_input, spanned::Spanned, ItemFn, PatType, Type};

/// Attribute macro that creates an additional version of a function that executes in a separate process.
///
/// When applied to a function named `foo`, this macro:
/// 1. Keeps the original function unchanged, allowing normal in-process calls
/// 2. Creates a new function named `foo_process` that returns a ProcessWrapper
///
/// # Requirements
///
/// The function must:
/// * Have arguments and return type that implement `Serialize` and `Deserialize`
///
#[proc_macro_error]
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

    let fn_name = &input_fn.sig.ident;
    let process_fn_name = format_ident!("{}_process", fn_name);
    let fn_args = &input_fn.sig.inputs;

    let fn_output = match &input_fn.sig.output {
        syn::ReturnType::Default => quote!(()),
        syn::ReturnType::Type(_, ty) => quote!(#ty),
    };

    // Check for mutable arguments
    for arg in fn_args.iter() {
        if let syn::FnArg::Typed(PatType { ty, .. }) = arg {
            if let Type::Reference(type_ref) = &**ty {
                if type_ref.mutability.is_some() {
                    Diagnostic::spanned(
                        ty.span().unwrap().into(),
                        Level::Warning,
                        "Mutable variables changes will not be reflected in the parent process."
                            .to_string(),
                    )
                    .emit();
                }
            }
        }
    }

    let mut self_stream = false;
    let arg_names: Vec<_> = fn_args
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let id = pat_ident.ident.clone();
                    Some(quote!(#id))
                } else {
                    panic!("Unsupported argument pattern")
                }
            }
            syn::FnArg::Receiver(_) => {
                self_stream = true;
                None
            }
        })
        .collect();

    let arg_types: Vec<_> = fn_args
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => pat_type.ty.clone(),
            syn::FnArg::Receiver(receiver) => {
                if let Some((_and_token, lifetime)) = &receiver.reference {
                    if receiver.mutability.is_some() {
                        syn::parse_quote!(&#lifetime mut Self)
                    } else {
                        syn::parse_quote!(&#lifetime Self)
                    }
                } else {
                    syn::parse_quote!(Self)
                }
            }
        })
        .collect();

    let args_types_tuple = quote! { (#(#arg_types),*) };
    let fn_name_str = fn_name.to_string();

    let call = if self_stream {
        quote!(self.#fn_name(#(#arg_names),*))
    } else {
        quote!(#fn_name(#(#arg_names),*))
    };

    let expanded = quote! {
        #input_fn

        #[allow(non_snake_case)]
        pub fn #process_fn_name(#fn_args) -> Result<process_fun::ProcessWrapper<#fn_output>, process_fun::ProcessFunError> {
            // Create pipes for result and start time communication
            #[cfg(feature = "debug")]
            eprintln!("[process-fun-debug] Creating pipes for process function: {}", #fn_name_str);

            let (mut read_pipe, mut write_pipe) = process_fun::create_pipes()?;

            // Fork the process
            #[cfg(feature = "debug")]
            eprintln!("[process-fun-debug] Forking process for function: {}", #fn_name_str);
            match process_fun::fork_process()? {
                process_fun::sys::ForkResult::Parent { child } => {
                    // Parent process - close write ends immediately
                    std::mem::drop(write_pipe);

                    // Create ProcessWrapper with child pid and receiver
                    Ok(process_fun::ProcessWrapper::new(child, read_pipe))
                }
                process_fun::sys::ForkResult::Child => {
                    // Child process - close read ends immediately
                    std::mem::drop(read_pipe);

                    #[cfg(feature = "debug")]
                    eprintln!("[process-fun-debug] Child process started");

                    // Get and send start time by stating the child process
                    let pid = process_fun::sys::getpid();
                    let start_time = process_fun::stat_pid_start(pid)?;
                    process_fun::write_time(&mut write_pipe, start_time)?;

                    #[cfg(feature = "debug")]
                    {
                        eprintln!("[process-fun-debug] Processing function: {}", &#fn_name_str);
                        eprintln!("[process-fun-debug] Arguments tuple type: {}", stringify!(#args_types_tuple));
                    }

                    // Execute the function with the original arguments
                    let result = #call;

                    #[cfg(feature = "debug")]
                    eprintln!("[process-fun-debug] Child process result: {:?}", &result);

                    // Serialize and write result
                    let result_bytes = process_fun::json::to_vec(&result)?;
                    process_fun::write_to_pipe(write_pipe, &result_bytes)?;

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

    #[cfg(feature = "debug")]
    {
        dbg!(expanded.to_string());
    }

    TokenStream::from(expanded)
}
