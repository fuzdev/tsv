<script lang="ts">
	// Short template - doesn't wrap
	const  short  =  `text1 text2`  ;

	// Long static template - tests wrapping behavior
	const  long1  =  `text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14`  ;

	// Long with interpolation
	const  long2  =  `text1 text2 text3 text4 text5 text6 text7 text8 ${ expr } text9 text10 text11 text12 text13`  ;

	// Long tagged template
	const  long3  =  tag`text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13`  ;
</script>
