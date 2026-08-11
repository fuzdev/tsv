<script lang="ts">
	declare namespace N {
		function* gen(): Iterator<number>;
	}

	declare module M {
		function* gen(): Iterator<number>;
	}

	declare global {
		function* gen(): Iterator<number>;
	}

	namespace P {
		function* gen(): Iterator<number>;
	}
</script>
