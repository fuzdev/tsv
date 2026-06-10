<script lang="ts">
// Inline `=` line comment is zero-width, so the constraint union stays inline.
type Default<P extends undefined|A,R extends A|B|void=// inline trailing comment that is intentionally long to exceed the width
// own-line comment about the default
C> = R;

// Two line comments after `=` stay on separate lines (inline + own-line), not merged.
type Mixed<U=// inline comment
// own-line comment
V> = U;

// Two line comments after `extends` stay on separate lines.
type Constraint<T extends// inline comment
// own-line comment
A> = T;
</script>
