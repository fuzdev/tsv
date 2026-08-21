<!-- A frozen body's interior whitespace is the author's own bytes: a trailing run, a blank
	line, a whitespace-only line and the authored indentation all ride out at every position. -->

<!-- Top-level: the writer emitter -->
<script  lang="coffee" >   
x = 1  
	y = 2	

   
z = 3   
   </script>

<!-- Nested: the doc emitter, whose body keeps column 0 -->
<div>
	<script  lang="coffee" >   
x = 1  
	y = 2	

   
z = 3   
	   </script>
	<style  lang="stylus" >   
a  
  color red	

   
  &:hover
    color blue   
	   </style>
</div>

<style  lang="sass" >   
a {  
		color: red;	

   
	/* c */
}   
   </style>
