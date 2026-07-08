<script lang="ts">
	interface A {
		[key /* c */ : string]: number;
	}

	interface B {
		[key: string /* ] */]: number;
	}

	type C = { [key: string /* ] */]: number };
</script>
