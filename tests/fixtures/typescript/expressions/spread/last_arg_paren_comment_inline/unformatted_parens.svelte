<script>
// same-line comment left behind by a spread's stripped parens: it rides the argument's
// line, and the line comment after the stripped `)` trails it
fn(a, ...(b /* i */) // t
);

new A(a, ...(b /* i */) // t
);

obj.method(a, ...(b /* i */) // t
);

// a line comment inside the stripped parens runs to end of line, so the argument list
// breaks around it — flushed inline, it would escape past the `)` and the `;`
fn(a, ...(b // i
));

new A(...(b // i
));
</script>
