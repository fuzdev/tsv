<script lang="ts">
// Optional tuple elements keep parens around lower-precedence operands
type Fn = [((() => void))?];
type Ctor = [((new () => T))?];
type Union = [((A | B))?];
type Inter = [((A & B))?];
type Cond = [((A extends B ? C : D))?];
type Keyof = [((keyof T))?];
</script>
