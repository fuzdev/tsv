<script lang="ts">
	// Short - stays on one line (contrast case)
	function short<T>(x: T): T { return x; }

	// Long single type param - stays inline, params break
	function longConstraint<T extends VeryLongConstraintTypeName | AnotherLongConstraintTypeName>(x: T): T { return x; }

	// Multiple long type params - each on own line
	function multipleLong<T extends LongConstraintOne | LongConstraintTwo, U extends LongConstraintThree | LongConstraintFour>(x: T, y: U): [T, U] { return [x, y]; }

	// Long with constraint and default - stays inline
	function longWithDefault<T extends VeryLongConstraintTypeName = VeryLongDefaultTypeName>(x: T): T { return x; }

	// Long object type constraint - wraps to multiple lines
	function longObjectConstraint<T extends {
			longPropertyName: VeryLongTypeName;
			anotherLongProperty: AnotherLongTypeName;
		}>(x: T): T { return x; }

	// Long function type constraint - wraps
	function longFnConstraint<T extends (
			longParam: VeryLongParamType,
			anotherParam: AnotherLongParamType,
		) => VeryLongReturnType>(f: T): T { return f; }

	// Long conditional type in default - wraps
	function longConditionalDefault<T, U extends T = T extends VeryLongConditionType ? VeryLongTrueType : never>(x: T) { return x; }

	// Async with long generic
	async function asyncLong<T extends VeryLongConstraintTypeName | AnotherLongConstraintTypeName>(url: string): Promise<T> { const response = await fetch(url); return response.json(); }

	// Generator with long generic
	function* generatorLong<T extends VeryLongConstraintTypeName | AnotherLongConstraintTypeName>(items: T[]): Generator<T> { for (const item of items) { yield item; } }
</script>
