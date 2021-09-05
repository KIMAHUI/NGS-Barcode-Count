# DEL-Decode
DNA encoded library decoding.  Multithreaded and low resource use.  Uses one thread to read and the rest to process the data, so at least a 2 threaded machine is essential.
This program does not store all data within RAM but instead sequentially processes the sequencing data in order to remain memory efficient.  
With very large DEL libraries, there may be some memory creep because it needs to count all occurances of building block barcodes and therefore needs to store any that are found.

## Table of Contents
<ul>
<li><a href=#Requirements>Requirements</a></li>
<li><a href=#files-needed>Files Needed</a></li>
<li><a href=#run>Run</a></li>
</ul>

## Requirements
<ul>
<li>Rust install: <a href=https://www.rust-lang.org/tools/install>instructions here</a></li>
</ul>

## Files Needed
Currently supports FASTQ, Sample Barcode, and Sequence Format.  Building block barcode and random barcode are still a work in progress.
<ul>
<li><a href=#fastq-file>FASTQ</a></li>
<li><a href=#sequence-format-file>Sequence format file</a></li>
<li><a href=#sample-barcode-file>Sample barcode file (optional)</a></li>
<li><a href=#building-block-barcode-file>Building block barcode file (optional)</a></li>
</ul>


### Fastq File
Currently only accepts unzipped FASTQ files due to this program reading line by line.<br>
Currently tryiing to implement a gzip inflate stream so that the whole file is not placed in RAM.

### Sequence Format File
The sequence format file should be a text file that is line separated by the type of format.  The following is supported where the '#' should be replaced by the number of nucleotides corresponding to the barcode:<br>
<table>
<tr>
<th>Sequence Type</th>
<th>File Code</th>
</tr>
<td>Constant</td>
<td>ATGC</td>
<tr>
<td>Sample Barcode</td>
<td>[#]</td>
</tr>
<tr>
<td>Building Block Barcode</td>
<td>{#}</td>
</tr>
</table>

An example can be found in [example.scheme.txt](example.scheme.txt)

### Sample Barcode File
<b>Optional</b><br>
The sample_barcode_file is a comma separate file with the following format:<br>
<table>
<tr>
<th>Barcode</th>
<th>Sample_ID</th>
</tr>
<tr>
<td>AGCATAC</td>
<td>Sample_name_1</td>
</tr>
<tr>
<td>AACTTAC</td>
<td>Sample_name_2</td>
</tr>
</table>

### Building Block Barcode File
<b>Optional</b><br>
The sample_barcode_file is a comma separate file with the following format:<br>
<table>
<tr>
<th>Barcode_1</th>
<th>Barcode_2</th>
<th>Barcode_3</th>
<th>Barcode_n</th>
<th>BB_ID</th>
</tr>
<tr>
<td>AGCATAC</td>
<td>TACAGCC</td>
<td>GCAGTGC</td>
<td>CAGAGAC</td>
<td>BB_name_1</td>
</tr>
<tr>
<td>AACTTAC</td>
<td>TTCAGAC</td>
<td>GGCAGGC</td>
<td>CAGACAC</td>
<td>BB_name_2</td>
</tr>
</table>
Where the last column is the ID and the first columns are continued for as many building blocks that exist.  The number of building blocks needs to match what
what is found in the sequence format file

## Run
Enter DEL-Encode directory and compile for the first time<br>
`$ cargo build --release`<br>
<br>
Run DEL-Decode<br>
`$ ./target/release/del --fastq <fastq_file> --sample_barcodes <sample_barcode_file> --sequence_format <sequence_format_file>`<br>
