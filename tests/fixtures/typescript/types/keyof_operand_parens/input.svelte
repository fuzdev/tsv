<script lang="ts">
	// keyof of a function type keeps its parens
	type Fn = keyof (() => void);

	// keyof of a constructor type keeps its parens
	type Ctor = keyof (new () => X);

	// keyof of a conditional type keeps its parens
	type Cond = keyof (A extends B ? X : Y);

	// keyof of a nested type operator keeps its parens
	type Nested = keyof (keyof X);

	// keyof of an infer type keeps its parens (inside a conditional extends)
	type Inf = [A] extends [keyof (infer U)] ? U : never;

	// keyof of a type query needs no parens
	type Query = keyof typeof obj;
</script>
