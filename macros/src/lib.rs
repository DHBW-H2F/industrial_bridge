use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parser, Ident, Token};

fn impl_into_hashmap(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

    let raw_fields = &match &ast.data {
        syn::Data::Struct(val) => val,
        _ => panic!("The IntoHashMap macro can only be used on a struct"),
    }
    .fields;

    let named_fields = match raw_fields {
        syn::Fields::Named(val) => val,
        _ => panic!("The IntoHashMap macro can only be used on named fields"),
    };

    let args = &ast
        .attrs
        .iter()
        .find_map(|attr| match attr.path().is_ident("implementation") {
            true => Some({
                let meta_list = match &attr.meta {
                    syn::Meta::List(list) => list,
                    _ => panic!("Wrong type for #[implementation(...)] parameters"),
                };
                let trait_ = syn::punctuated::Punctuated::<Ident, Token![,]>::parse_terminated
                    .parse(meta_list.tokens.clone().into())
                    .expect("Invalid #[implementation(Trait, Error)] attribute entered");
                trait_
            }),
            false => None,
        })
        .expect("No #[implementation(Trait, Error)]");

    let type_: Ident = args
        .first()
        .expect("#[implementation(Trait, Error)] could not parse Trait")
        .clone();
    let error_: Ident = args
        .get(1)
        .expect("#[implementation(Trait, Error)] could not parse Error")
        .clone();

    let type_map: Vec<(Ident, Ident)> = named_fields
        .named
        .iter()
        .filter_map(|f| {
            if f.ident.is_none() {
                return None;
            }

            let attrs = f
                .attrs
                .iter()
                .find_map(|attr| match attr.path().is_ident("device") {
                    true => Some({
                        let device: Ident = attr
                            .parse_args()
                            .expect("Invalid #[device(...) attribute entered]");
                        device
                    }),
                    false => None,
                })
                .expect(
                    "#[derive(FiniteStateMachine)] need the #[device(...)] attribute on each field",
                );

            Some((f.ident.clone().unwrap(), attrs))
        })
        .collect();

    let (fields, typ): (Vec<Ident>, Vec<Ident>) = type_map.into_iter().unzip();

    let gen = quote! {
        impl TryInto<HashMap<String, Box<dyn #type_ + Send>>> for #name {
            type Error = #error_;

            fn try_into(self) -> Result<HashMap<String, Box<dyn #type_ + Send>>, #error_> {
                let mut res: HashMap<String, Box<dyn #type_ + Send>> = HashMap::new();

                #(
                match self.#fields {
                    Some(field) => {
                        for (name, dev_def) in field {
                            let dev: Box<#typ> = Box::new(dev_def.try_into()?);
                            res.insert(name, dev);
                        }
                    },
                    None => {
                    }
                };)*
                Ok(res)
            }
        }
    };
    gen.into()
}

#[proc_macro_derive(IntoHashMap, attributes(device, implementation))]
pub fn instanciate_device_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_into_hashmap(&ast)
}
