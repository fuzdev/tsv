<style>
.card{padding:20px;background:white;

.header{font-size:24px;

&:hover{color:blue;}}

& .footer{font-size:12px;}

& > .content{margin:10px 0;

@media (min-width: 768px){margin:20px 0;}}

&.active{border:2px solid blue;

@supports (box-shadow: 0 0 5px blue){box-shadow:0 0 5px blue;}}}
</style>
