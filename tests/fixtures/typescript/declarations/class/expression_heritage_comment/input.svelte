<script lang="ts">
	// Line comment between name and extends
	let a = class A // comment
		extends B {};

	// Block comment between name and extends
	let b = class C /* comment */ extends B {};
</script>
