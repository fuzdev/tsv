<script lang="ts">
	// MemberExpression extends with long type params - extends inline after >
	class InlineExtends<
		T extends VeryLongTypeName | AnotherLongTypeName,
		U extends ExtraLongConstraintType | MoreTypes,
	> extends A.B<T> {
		value: T;
	}

	// MemberExpression extends with long type params and implements - all break
	class BothBreak<
		T extends VeryLongTypeName | AnotherLongTypeName,
		U extends ExtraLongConstraintType | MoreTypes,
	>
		extends A.B<T>
		implements Contract<U>
	{
		a: T;
		b: U;
	}

	// Short - MemberExpression extends on one line (contrast case)
	class Short<T> extends A.B<T> {}
</script>
