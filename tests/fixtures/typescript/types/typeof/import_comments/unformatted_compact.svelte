<script lang="ts">
// leading line comment - breaks to multiline
type A = typeof import(
// c
'a');

// leading block comment on its own line - breaks
type B = typeof import(
/* c */
'a');

// leading block comment inline - stays inline
type C = typeof import(/* c */'a');

// trailing line comment - breaks to multiline
type D = typeof import('a'// c
);

// trailing block comment inline - stays inline
type E = typeof import('a'/* c */);

// leading comment with qualifier - stays inline
type F = typeof import(/* c */'a').Foo;

// leading comment with qualifier and type arguments
type G = typeof import(/* c */'a').Foo<string>;

// trailing block comment with qualifier - stays inline
type H = typeof import('a'/* c */).Foo;
</script>
