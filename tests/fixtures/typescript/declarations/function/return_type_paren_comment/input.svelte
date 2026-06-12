<script lang="ts">
	// function declaration - empty params
	function f() /* c */ : number {
		return 1;
	}

	// function declaration - with param
	function g(a) /* c */ : number {
		return a;
	}

	// function expression
	const h = function (a) /* c */ : number {
		return a;
	};

	// class method and getter
	class A {
		m(a) /* c */ : number {
			return a;
		}
		get x() /* c */ : number {
			return 1;
		}
	}
</script>
