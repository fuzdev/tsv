<script lang="ts">
	class A {
		constructor(
			@dec  
			private   a  :  number
		) {}

		fn1(
			@dec
			b  :  number
		) {}

		fn2(
			@dec1  
			@dec2
			c  :  number
		) {}

		fn3(
			@dec
			{  d  }  :  T
		) {}

		fn4(
			@dec1   e  :  number,
			@dec2
			f  :  number
		) {}
	}
</script>
