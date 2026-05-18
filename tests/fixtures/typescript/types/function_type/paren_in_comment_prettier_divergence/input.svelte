<script lang="ts">
	// function type - comment with parens between type params and (
	type A = <T> /* a(b) */(x: T) => void;

	// constructor type - comment with parens between type params and (
	type B = new <T> /* a(b) */(x: T) => A;
</script>
