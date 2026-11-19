// SPDX-License-Identifier: MPL-2.0
//! The `#[gnu2_tls]` attribute. Use it through the `gnu2-tls` crate, which
//! re-exports it.

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

/// Declares a thread local that is reached through a GNU2 TLS descriptor.
///
/// See the `gnu2-tls` crate docs for details.
#[proc_macro_attribute]
pub fn gnu2_tls(attr: TokenStream, item: TokenStream) -> TokenStream {
	if let Some(token) = attr.into_iter().next() {
		return compile_error("#[gnu2_tls] takes no arguments", token.span());
	}
	// the actual parsing lives in a macro_rules in the runtime crate, so this
	// crate doesn't need syn
	macro_call(["gnu2_tls", "__declare"], item, Span::call_site())
}

fn compile_error(message: &str, span: Span) -> TokenStream {
	let mut literal = Literal::string(message);
	literal.set_span(span);
	macro_call(
		["core", "compile_error"],
		TokenTree::from(literal).into(),
		span,
	)
}

/// Builds `::first::second! { body }`.
fn macro_call(path: [&str; 2], body: TokenStream, span: Span) -> TokenStream {
	let mut tokens: Vec<TokenTree> = Vec::with_capacity(8);
	for segment in path {
		tokens.push(Punct::new(':', Spacing::Joint).into());
		tokens.push(Punct::new(':', Spacing::Alone).into());
		tokens.push(Ident::new(segment, span).into());
	}
	tokens.push(Punct::new('!', Spacing::Alone).into());
	tokens.push(Group::new(Delimiter::Brace, body).into());
	tokens.into_iter().collect()
}
