<script lang="ts">
	// Class decorators

	// 100 chars exactly - stays inline (at boundary)
	@decorator({prop1: 'value1', prop2: 'value2', prop3: 'value3', prop4: 'value44444444444444444'})
	class A {}

	// 101 chars - wraps to one property per line
	@decorator({prop1: 'value1', prop2: 'value2', prop3: 'value3', prop4: 'value444444444444444444'})
	class B {}

	// Method decorators (at indent level 2)

	class C {
		// 100 chars exactly - stays inline (at boundary)
		@decorator({prop1: 'value1', prop2: 'value2', prop3: 'value3', prop4: 'value444444444444444'})
		method1() {}

		// 101 chars - wraps to one property per line
		@decorator({prop1: 'value1', prop2: 'value2', prop3: 'value3', prop4: 'value4444444444444444'})
		method2() {}
	}
</script>
