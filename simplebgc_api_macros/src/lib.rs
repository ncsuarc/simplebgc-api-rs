extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn;

// TODO: Figure out path handling
// TODO: Add range validation
// TODO: Improve error messages

fn to_token(i: usize, f: &syn::Field) -> impl ToTokens {
    match &f.ident {
        Some(ident) => ident.to_token_stream(),
        None => syn::Index::from(i).to_token_stream(),
    }
}

#[proc_macro_derive(Transmit, attributes(range))]
pub fn command_part_derive(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree that we can manipulate
    let ast: syn::DeriveInput = syn::parse(input).unwrap();

    let name = &ast.ident;

    let gen = match &ast.data {
        syn::Data::Struct(data) => {
            let fields: Vec<_> = data
                .fields
                .iter()
                .enumerate()
                .map(|(i, f)| to_token(i, f))
                .collect();

            // TODO: Collect as many errors as possible by not panicking
            let types = data.fields.iter().map(|field| &field.ty);

            // TODO: Handle ranges
            let checks = data.fields.iter().enumerate().map(|(i, f)| {
                let ident = to_token(i, f);
                let check_this = f
                    .attrs
                    .iter()
                    .filter(|attr| attr.path.is_ident("range"))
                    .map(|attr| {
                        use syn::punctuated::Punctuated;
                        let parser = Punctuated::<syn::ExprRange, syn::Token![,]>::parse_terminated;
                        attr.parse_args_with(parser).unwrap()
                    })
                    .map(|punctuated| {
                        let ranges = punctuated.iter();
                        quote! {
                            if #(!(#ranges).contains(&self.#ident))&&* {
                                return Err(::std::io::Error::new(::std::io::ErrorKind::InvalidData, "data outside of valid range"));
                            }
                        }
                    });
                quote!(#(#check_this)*)
            });

            quote! {
                impl Transmit for #name {
                    fn validate(&self) -> ::std::io::Result<()> {
                        #(#checks)*
                        Ok(())
                    }
                    fn from_reader<R: ::byteorder::ReadBytesExt>(reader: &mut R) -> ::std::io::Result<Self> {
                        // Safety: We immediately fill in all fields for the struct, guaranteed by
                        let mut data: Self = unsafe {
                            ::std::mem::MaybeUninit::uninit().assume_init()
                        };
                        #( data.#fields = <#types>::from_reader(reader)?; )*
                        data.validate()?;
                        Ok(data)
                    }
                    fn to_writer<W: ::byteorder::WriteBytesExt>(&self, writer: &mut W) -> ::std::io::Result<()> {
                        self.validate()?;
                        #( self.#fields.to_writer(writer)?; )*
                        Ok(())
                    }
                }
            }
        }

        syn::Data::Enum(data) => {
            let ty = ast
                .attrs
                .iter()
                .filter(|attr| attr.path.is_ident("repr"))
                .map(|attr| attr.parse_args::<syn::TypePath>().unwrap())
                .next()
                .expect("derive(Transmit) requires a #[repr(T)] attribute for enums");
            let variants: Vec<_> = data.variants.iter().map(|variant| &variant.ident).collect();
            let discriminants: Vec<_> = data
                .variants
                .iter()
                .map(|variant| {
                    let (_eq, expr) = variant.discriminant.as_ref().unwrap();
                    expr
                })
                .collect();

            quote! {
                impl Transmit for #name {
                    // Enums can only be valid values
                    fn validate(&self) -> ::std::io::Result<()> {
                        Ok(())
                    }
                    fn from_reader<R: ::byteorder::ReadBytesExt>(reader: &mut R) -> ::std::io::Result<Self> {
                        // Safety: We immediately fill in all fields for the struct, guaranteed by
                        match <#ty>::from_reader(reader)? {
                            #( #discriminants => Ok(Self::#variants), )*
                            _ => Err(::std::io::Error::new(::std::io::ErrorKind::InvalidData, "read value does not match any enum variants")),
                        }
                    }
                    fn to_writer<W: ::byteorder::WriteBytesExt>(&self, writer: &mut W) -> ::std::io::Result<()> {
                        let val = match self { #( Self::#variants => #discriminants, )* };
                        <#ty>::to_writer(&val, writer)
                    }
                }
            }
        }

        syn::Data::Union(_) => panic!("derive(Transmit) does not work on unions"),
    };

    gen.into()
}

#[proc_macro_derive(Command, attributes(id))]
pub fn command_derive(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree that we can manipulate
    let ast: syn::DeriveInput = syn::parse(input).unwrap();

    let name = &ast.ident;
    let id: u8 = ast
        .attrs
        .iter()
        .filter(|attr| attr.path.is_ident("id"))
        .map(|attr| attr.parse_args::<syn::LitInt>().unwrap())
        .map(|lit| lit.base10_parse().unwrap())
        .next()
        .expect("derive(Command) requires a #[id(N)] attribute");

    // TODO: Should I actually get rid of the common thing and enforce that all commands are
    // command parts? Probably. That will require renaming the trait to reflect it's usage though.
    let gen = quote! {
        impl Command for #name where Self: Transmit {
            const ID: u8 = #id;
            fn parse_payload<R: ::byteorder::ReadBytesExt>(reader: &mut R) -> ::std::io::Result<Self> {
                Self::from_reader(reader)
            }
            fn write_payload<W: ::byteorder::WriteBytesExt>(&self, writer: &mut W) -> ::std::io::Result<()> {
                self.to_writer(writer)
            }
        }
    };

    gen.into()
}
