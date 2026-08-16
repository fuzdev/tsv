<script lang="ts">
	// A break inside the type parameters does not break the parameter list
	const a = function <
		// c1
		T extends A
	>(x: T) {
		call(x);
	};

	// An object method takes the same path
	const o = {
		m<
			// c2
			T extends A
		>(x: T) {
			call(x);
		}
	};

	// A function declaration and a class method are the same shape
	function fn1<
		// c3
		T extends A
	>(x: T) {
		call(x);
	}
	class C {
		m<
			// c4
			T extends A
		>(x: T) {
			call(x);
		}
	}
</script>
