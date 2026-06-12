<script lang="ts">
	// Constrained infer at the top-level check (parens not needed — TS reads this as the constraint)
	type Unwrap<T> = T extends (infer U extends string) ? U : never;

	// Inside a type argument
	type ArrayEl<T> = T extends Array<(infer Item extends number)> ? Item : never;

	// Inside a tuple element
	type First<T> = T extends [(infer F extends string), ...any[]] ? F : never;

	// Constraint can be an operator type
	type KeyOf<T> = T extends (infer K extends keyof T) ? K : never;

	// Constraint can be a union
	type Wide<T> = T extends (infer U extends string | number) ? U : never;

	// Multiple constrained infers in an object type
	type Both<T> = T extends {a: (infer A extends string); b: (infer B extends number)} ? [A, B] : never;
</script>
