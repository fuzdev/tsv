<script lang="ts">
	// Long type params with extends - params break, extends inline
	abstract class LongExtends<
		T extends VeryLongTypeName | AnotherLongTypeName,
		U extends ExtraLongConstraintType | MoreTypes,
	> extends Base<T> {
		abstract value: T;
	}

	// Long type params with implements - implements breaks, brace on new line
	abstract class LongImplements<
		T extends VeryLongTypeName | AnotherLongTypeName,
		U extends ExtraLongConstraintType | MoreTypes,
	>
		implements Interface1<T>, Interface2<U>
	{
		abstract a: T;
		abstract b: U;
	}

	// Long with both extends and implements - all break
	abstract class LongBoth<
		T extends VeryLongTypeName | AnotherLongTypeName,
		U extends ExtraLongConstraintType | MoreTypes,
	>
		extends Base<T>
		implements Interface<U>
	{
		abstract prop: string;
	}

	// Short - stays on one line (70 chars, contrast case)
	abstract class Short<T extends string = string, U extends number = number> {
		abstract a: T;
		abstract b: U;
	}
</script>
