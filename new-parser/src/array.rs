use crate::priv_prelude::*;

#[derive(Debug, Clone)]
pub struct ArrayRepeatDescriptor<T> {
    pub elem: Box<T>,
    pub semicolon_token: SemicolonToken,
    pub len: Box<Expr>,
}

impl<T> Spanned for ArrayRepeatDescriptor<T>
where
    T: Spanned,
{
    fn span(&self) -> Span {
        Span::join(self.elem.span(), self.len.span())
    }
}

pub fn array_repeat_descriptor<P, T>(
    elem_parser: P,
) -> impl Parser<Output = ArrayRepeatDescriptor<T>> + Clone
where
    T: Spanned,
    P: Parser<Output = T> + Clone,
{
    elem_parser
    .then_optional_whitespace()
    .then(semicolon_token())
    .then_optional_whitespace()
    .then(lazy(|| expr()).map(Box::new))
    .map(|((elem, semicolon_token), len)| {
        ArrayRepeatDescriptor {
            elem: Box::new(elem),
            semicolon_token,
            len,
        }
    })
}
