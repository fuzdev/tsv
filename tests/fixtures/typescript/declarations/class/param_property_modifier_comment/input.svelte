<script lang="ts">
	// readonly modifier (decorator-free) — the minimal trigger
	class A {
		constructor(readonly /* c */ a) {}
	}

	// accessibility modifier
	class B {
		constructor(private /* c */ b) {}
	}

	// accessibility + readonly + type annotation (comment after last modifier)
	class C {
		constructor(public readonly /* c */ c: number) {}
	}

	// override modifier with a default value
	class D {
		constructor(protected override /* c */ d = 1) {}
	}

	// decorated parameter property (decorator precedes the modifiers)
	class E {
		constructor(@dec readonly /* c */ e) {}
	}
</script>
