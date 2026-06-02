<script lang="ts">
// Nested union keeps its parens (A | (B | C) does not flatten to A | B | C)
type Short = X | ((Y | Z));

// Parens preserved in the middle
type Mid = X | ((Y | Z)) | W;

// Parens preserved at the start
type Start = ((Y | Z)) | W;

// Outer union breaks; inner parenthesized union still fits inline
type Long =
	foooooooooooooooooooooooooooooooooooo | ((barrrrrrrrrrrrrrrrrrrrrrrrrr | bazzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz | quxxxxxxxxxxx));
</script>
