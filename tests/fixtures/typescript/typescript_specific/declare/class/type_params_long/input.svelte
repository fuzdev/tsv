<script lang="ts">
	// Long type params with both extends and implements - all break
	declare class LongBoth<
		T extends VeryLongTypeName | AnotherLongTypeName,
		U extends ExtraLongConstraintType | MoreTypes,
	>
		extends Base<T>
		implements Interface<U>
	{
		prop: string;
	}

	// Short - stays on one line (77 chars, contrast case)
	declare class Short<T extends string = string, U extends number = number> {
		a: T;
		b: U;
	}

	// Long extends only - extends stays inline after >
	declare class LongExtends<
		T extends VeryLongTypeName | AnotherLongTypeName,
		U extends ExtraLongConstraintType | MoreTypes,
	> extends Base<T> {
		value: T;
	}

	// Long implements only - implements breaks with brace on new line
	declare class LongImplements<
		T extends VeryLongTypeName | AnotherLongTypeName,
		U extends ExtraLongConstraintType | MoreTypes,
	>
		implements Interface1<T>, Interface2<U>
	{
		a: T;
		b: U;
	}
</script>
