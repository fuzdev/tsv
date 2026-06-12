<script lang="ts">
	class A {
		static [key: string]: number;
	}

	class B {
		static readonly [key: string]: number;
	}

	class C {
		readonly [key: string]: number;
		static [idx: number]: string;
	}
</script>
