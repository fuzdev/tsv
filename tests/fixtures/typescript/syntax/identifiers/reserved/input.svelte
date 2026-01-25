<script lang="ts">
	//
	// Reserved words in object literal positions (ES5+)
	//

	const obj = {
		// Control flow keywords
		if: 1,
		else: 2,
		for: 3,
		while: 4,
		do: 5,
		switch: 6,
		case: 7,
		break: 8,
		continue: 9,
		return: 10,
		throw: 11,
		try: 12,
		catch: 13,
		finally: 14,

		// Operator keywords
		new: 15,
		delete: 16,
		typeof: 17,
		void: 18,
		instanceof: 19,
		in: 20,

		// Declaration keywords
		function: 21,
		var: 22,
		let: 23,
		const: 24,

		// Literal keywords
		true: 25,
		false: 26,
		null: 27,

		// ES6+ keywords
		import: 28,
		export: 29,
		extends: 30,
		static: 31,
		class: 32,
		super: 33,
		this: 34,
		async: 35,
		await: 36,
		yield: 37,

		// TypeScript keywords
		enum: 38,
		type: 39,
		interface: 40,

		// Methods with reserved names
		default() {
			return 0;
		},
		get with() {
			return 1;
		},
		set with(v) {},
	};

	// Destructuring with reserved word keys
	const {if: a, else: b, class: c, default: d} = obj;

	// Member access with reserved words
	const e = obj.if;
	const f = obj.class;
	obj.else = 100;

	//
	// Reserved words as class members
	//

	class A {
		// JS reserved words as method names
		delete(): void {}
		default(): number {
			return 0;
		}

		// Contextual keywords as parameter names
		fn1(from: string, to: string): void {}

		// JS reserved words as property names
		extends = 1;
		static = 2;

		// JS reserved words as accessor names
		get class(): number {
			return 0;
		}
		set class(v: number) {}
	}

	class B {
		// Async/generator/static with reserved name
		async delete(): Promise<void> {}
		*default(): Generator<number> {
			yield 1;
		}
		static extends(): void {}
	}

	//
	// Strict mode reserved words as class members
	// (Valid as property/method names, NOT as parameter names)
	//

	class C {
		// As property names
		private = 1;
		protected = 2;
		public = 3;
		let = 4;
		const = 5;
		var = 6;
		enum = 7;
		yield = 8;
		await = 9;

		// As method names
		private(): void {}
		protected(): void {}
		public(): void {}
	}

	//
	// TypeScript contextual keywords as class members
	//

	class D {
		// As property names
		type = 1;
		interface = 2;

		// As method names
		implements(): void {}
		package(): void {}
	}

	//
	// TypeScript modifier keywords as class members
	// (readonly, override, abstract, accessor, static, async)
	//

	class E {
		// As property names
		readonly = 1;
		override = 2;
		abstract = 3;
		accessor = 4;
		static = 5;
		async = 6;

		// As method names
		readonly(): void {}
		override(): void {}
		abstract(): void {}
		accessor(): void {}
	}
</script>
