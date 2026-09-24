<script>
// own-line comment left behind by a spread's stripped parens, on the last argument
new A(a, ...(b
/* i */
));

obj.method(a, ...(b
/* i */
));

// same, with a line comment on its own line after the stripped `)`
new A(a, ...(b
/* i */
)
// t
);

obj.method(a, ...(b
/* i */
)
// t
);

// an own-line line comment left behind, then a comment on its own line after the stripped
// `)`: each keeps its own line, in the order the author wrote them
fn(a, ...(b
// i
)
/* t */
);

new A(a, ...(b
// i
)
/* t */
);

obj.method(a, ...(b
// i
)
/* t */
);

fn(a, ...(b
// i
)
// t
);

new A(a, ...(b
// i
)
// t
);

obj.method(a, ...(b
// i
)
// t
);
</script>
