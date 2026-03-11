<script lang="ts">
	// 100 chars - stays flat, no parens
	function fn(): string {return a+b+c+d+e+f+g+h+i+j+k+l+m+n+o+p+q+r+s+t+u+vvvv;}

	// 101 chars - breaks with parens
	function fn1(): string {return a+b+c+d+e+f+g+h+i+j+k+l+m+n+o+p+q+r+s+t+u+vvvvv;}

	// Short control - no parens
	function fn2(): number {return a+b+c+d;}

	// Throw also wraps binary chains in parens when breaking (101 chars)
	function fn3(): never {throw a+b+c+d+e+f+g+h+i+j+k+l+m+n+o+p+q+r+s+t+u+vvvvvv;}
</script>
