<script lang="ts">
	// Function overload signature
	function fn1<T>/* c */(x: T): T;
	function fn1(x: any): any {
		return x;
	}

	// Abstract class method
	abstract class A {
		abstract m<T>/* c */(x: T): void;
	}
</script>
