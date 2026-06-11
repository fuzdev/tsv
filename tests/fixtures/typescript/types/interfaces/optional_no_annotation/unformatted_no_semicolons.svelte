<script lang="ts">
	// Optional members without type annotation (implicit any)
	interface A {
		a?
		readonly b?
		'k'?
		[c]?
		m?()
	}

	// Same shape in a type literal
	type T = {
		c?
		d?
	};
</script>
