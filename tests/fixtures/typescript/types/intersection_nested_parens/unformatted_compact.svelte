<script lang="ts">
// Nested intersection keeps its parens (A & (B & C) does not flatten to A & B & C)
type Short=A&(B&C);

// Parens preserved in the middle
type Mid=A&(B&C)&D;

// Parens preserved at the start
type Start=(B&C)&D;

// Outer intersection breaks; inner parenthesized intersection still fits inline
type Long=foooooooooooooooooooooooooooooooooooo&(barrrrrrrrrrrrrrrrrrrr&bazzzzzzzzzzzzzzzzzzz);
</script>
