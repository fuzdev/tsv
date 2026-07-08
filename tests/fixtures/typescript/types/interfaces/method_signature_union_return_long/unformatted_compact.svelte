<script lang="ts">
	// An over-long interface method signature breaks its PARAMETERS, keeping the
	// breakable-union generic return type inline — the same layout a class method
	// and prettier use (params break before the return-type generic does).

	// 100 chars - stays inline
	interface A {
methodName(paramName: TypeNameAAAAAAAA, otherName: OtherTypeAAAAAAAA): Resolvable<TypeA | null>;
	}

	// 101 chars - params break, union return stays inline
	interface B {
methodName(paramName: TypeNameAAAAAAAAA, otherName: OtherTypeAAAAAAAA): Resolvable<TypeA | null>;
	}
</script>
