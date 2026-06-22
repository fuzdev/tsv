<script lang="ts">
	// Long type params - params break
	declare function longParams<
		T extends LongConstraintOne | LongConstraintTwo,
		ES extends LongConstraintThree | LongConstraintFour
	>(base: T, signals: ES): Config<T, ES>;

	// Short - stays on one line (97 chars, contrast case)
	declare function short<T extends string = string, U extends number = number>(x: T, y: U): void;

	// Very long constraints
	declare function veryLong<
		T extends VeryLongTypeName | AnotherLongTypeName,
		U extends ExtraLongConstraintType | MoreTypes
	>(a: T, b: U): [T, U];
</script>
