<!-- a spread's stripped parens leave an own-line comment behind in a template expression
too; a comment written after the `)` follows it, in the order the author wrote them -->
<p>{[a, ...(b
// i
) /* t */]}</p>

<p>{fn(a, ...(b
/* i */
) /* t */)}</p>
