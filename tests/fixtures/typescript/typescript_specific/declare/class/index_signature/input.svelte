<script lang="ts">
	declare class A {
		[key: string]: number;
	}

	declare class B {
		readonly [key: string]: number;
	}

	declare class C {
		static [key: string]: number;
	}

	declare class D {
		static readonly [key: string]: number;
		[idx: number]: string;
	}
</script>
