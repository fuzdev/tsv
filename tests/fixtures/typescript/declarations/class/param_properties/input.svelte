<script lang="ts">
	class A {
		constructor(public x: number) {}
	}

	class B {
		constructor(
			private readonly y: string,
			protected z: boolean
		) {}
	}

	class C {
		constructor(
			public a: number,
			private b: string,
			c: boolean
		) {}
	}

	class D {
		constructor(
			public x = 1,
			private y = 'default',
			protected z: boolean = true
		) {}
	}

	class E {
		constructor(public readonly items: string[] = []) {}
	}
</script>
