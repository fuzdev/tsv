<script lang="ts">
	class A {
		// String index
		[key: string]: number;
	}

	class B {
		// Number index
		[idx: number]: string;
	}

	class C {
		// Symbol index
		[sym: symbol]: boolean;
	}
</script>
