use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{ItemFn, FnArg, Type, Pat, PatType, Path, Ident};

/// Creates a test that generates a corresponding TypeScript interface for this struct. To generate TypeScript bindings, run ```cargo test```
/// **Important:** In order for this macro to work, both ts_rs and serde need to be in scope. This can be achieved by importing the prelude: ```use tauri_bindgen_ts::prelude::*```
///
/// By default, the location is set to "../src-gen" which results in a top-level directory "src-gen in your Tauri app.
/// A different output directory can be specified by passing a path as string argument, i.e. ```#[entity("./my-custom-dir)"] struct MyStruct { }```
#[proc_macro_attribute]
pub fn entity(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item: proc_macro2::TokenStream = item.into();
    let dir = format!("{}/", parse_dir_arg(&attr));

    quote! {
        #[derive(ts_rs::TS, serde::Serialize, serde::Deserialize)]
        #[ts(export)]
        #[ts(export_to=#dir)]
        #item
    }.into()
}

/// Turns this function into a Tauri command and creates a test that generates a TypeScript binding to this function. To generate TypeScript bindings, run ```cargo test```
/// **Important:** In order for this macro to work, both ts_rs and serde need to be in scope. This can be achieved by importing the prelude: ```use tauri_bindgen_ts::prelude::*```
///
/// By default, the location is set to "../src-gen" which results in a top-level directory "src-gen in your Tauri app.
/// A different output directory can be specified by passing a path as string argument, i.e. ```#[entity("./my-custom-dir)"] struct MyStruct { }```
#[proc_macro_attribute]
pub fn command(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = syn::parse::<ItemFn>(item.clone()).expect("This attribute should be used on a function!");
    let item: proc_macro2::TokenStream = item.into();

    let dir = parse_dir_arg(&attr);
    let func = func_metadata(func);
    let test = generate_test(func, dir);

    quote! {
        #[tauri::command]
        #item
        #test
    }.into()
}


/// Parse the specified export dir from attributes. Defaults to "../src-gen"
fn parse_dir_arg(attr: &TokenStream) -> String {
    // TODO: Validate path
    let dir = attr.to_string();
    let dir = dir.trim_matches(|c| c == '"' || c == '\'' );
    if dir.is_empty() { "../src-gen".to_owned() } else { dir.to_owned() }
}

struct Func {
    name: String,
    args: Vec<(Ident, Path)>,
}

fn func_metadata(func: ItemFn) -> Func {
    let name = func.sig.ident.to_string();
    // TODO: Implement mechanism to skip args such as tauris app handle (can be done with attrs)
    let args = func.sig.inputs.into_iter()
        .filter_map(|arg| if let FnArg::Typed(t) = arg { Some(t) } else { panic!("Only top-level functions are allowed as commands!") })
        .collect::<Vec<_>>();
    // TODO: Support more function arg types
    let args = types(&args);

    Func { name, args }
}

fn types(args: &[PatType]) -> Vec<(Ident, Path)> {
    args.iter()
        .map(|arg| (arg.pat.clone(), arg.ty.clone()))
        .filter_map(|(pat, ty)| match *pat {
            Pat::Ident(p) => Some((p, ty)),
            _ => panic!("Only simple owned types are allowed as arguments at the moment!"),
        })
        .filter_map(|(pat, ty)| match *ty {
            Type::Path(t) => Some((pat, t)),
            _ => { panic!("Only simple owned types are allowed as arguments at the moment!") }
        })
        .map(|(pat, ty)| (pat.ident, ty.path))
        .collect()
}

/// * `func`- An object that holds a functions metadata such as name and arguments
/// * `dir` - Directory to which the resulting file will be exported
fn generate_test(func: Func, dir: String) -> proc_macro2::TokenStream {
    let Func { name, args } = func;
    let arg_names = args.iter().map(|(ident, _)| ident.to_string()).collect::<Vec<_>>();
    let arg_types = args.iter().map(|(_, path)| path).collect::<Vec<_>>();

    let test_fn = format_ident!("export_function_bindings_{}", name);

    let header = "// This file was generated by [tauri-bindgen-ts](https://github.com/antoniusnaumann/tauri-bindgen-ts). Do not edit this file manually.";
    // TODO: Also import argument types
    let import = "import { invoke } from \"@tauri-apps/api/tauri\"";
    let binding = format!("export async function {name}(%0) {{ return await invoke('{name}', {{ %1 }}) }}");

    let file_name = format!("{dir}/{name}.ts");
    let content = format!("{header}\n{import}\n\n{binding}");

    quote! {
        #[cfg(test)]
        #[test]
        fn #test_fn() {
            use std::fs;
            use tauri_bindgen_ts::ts_rs::TS;

            let types = vec![#(#arg_types::name()),*];
            let names = vec![#(#arg_names),*];
            let args = types.iter().enumerate().map(|(index, elem)| [names[index].to_owned(), elem.to_owned()].join(": ")).collect::<Vec<String>>().join(", ");

            fs::create_dir_all(#dir).expect("Could not create directory");
            fs::write(#file_name, #content.replace("%0", args.as_str()).replace("%1", names.join(", ").as_str())).expect("Could not write generated function binding to file");
        }
    }
}