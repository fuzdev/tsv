<style>
div{
/* trailing comments after declarations directly inside an at-rule block stay on the line */
@media screen{
color:red;/* comment1 */
background:blue;/* comment2 */
font-size:12px;/* comment3 */
}
}

span{
@supports (display:grid){
width:1px;/* comment4 */
}
}
</style>
