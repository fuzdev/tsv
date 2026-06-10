<script lang="ts">
class Foo {
// Two line comments after `=` stay on separate lines (inline + own-line), not merged.
a=// inline comment
// own-line comment
value;

// Inline `=` line comment is zero-width, so a preceding type union stays inline.
b:A|B|void=// inline trailing comment that is intentionally long to exceed the print width
value;

// Own-line comment after `=` stays on its own line.
c=
// own-line comment
value;
}
</script>
