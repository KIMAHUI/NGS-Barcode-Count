use regex::Captures;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::atomic::Ordering,
};

type CountedBarcode = String;
type BarcodeID = String;
type BarcodeBarcodeID = HashMap<CountedBarcode, BarcodeID>;
type BarcodeNumBarcode = Vec<BarcodeBarcodeID>;

pub struct SequenceParser {
    shared_mut_clone: crate::barcode_info::SharedMutData,
    sequence_errors_clone: crate::barcode_info::SequenceErrors,
    sequence_format_clone: crate::barcode_info::SequenceFormat,
    samples_clone: Option<HashMap<String, String>>,
    barcodes_clone: Option<BarcodeNumBarcode>,
    max_errors_clone: crate::barcode_info::MaxSeqErrors,
    sample_seqs: HashSet<String>,
    barcode_seqs: Vec<HashSet<String>>,
    raw_sequence: RawSequence,
    barcode_groups: Vec<String>,
}

impl SequenceParser {
    pub fn new(
        shared_mut_clone: crate::barcode_info::SharedMutData,
        sequence_errors_clone: crate::barcode_info::SequenceErrors,
        sequence_format_clone: crate::barcode_info::SequenceFormat,
        samples_clone: Option<HashMap<String, String>>,
        barcodes_clone: Option<BarcodeNumBarcode>,
        max_errors_clone: crate::barcode_info::MaxSeqErrors,
    ) -> SequenceParser {
        let mut barcode_groups = Vec::new();
        for x in 0..sequence_format_clone.barcode_num {
            barcode_groups.push(format!("barcode{}", x + 1))
        }
        SequenceParser {
            shared_mut_clone,
            sequence_errors_clone,
            sequence_format_clone,
            samples_clone,
            barcodes_clone,
            max_errors_clone,
            sample_seqs: HashSet::new(),
            barcode_seqs: Vec::new(),
            raw_sequence: RawSequence::new(String::new()),
            barcode_groups,
        }
    }
    pub fn parse(&mut self) -> Result<(), Box<dyn Error>> {
        self.get_sample_seqs();
        // Get a vec of all possible building block barcodes for error correction
        self.get_barcode_seqs();

        // Loop until there are no sequences left to parse.  These are fed into seq vec by the reader thread
        loop {
            if self.get_seqeunce() {
                if let Some(seq_match_result) = self.match_seq()? {
                    let barcode_string = seq_match_result.barcode_string();
                    // If there is a random barcode included
                    if seq_match_result.random_barcode.is_empty() {
                        self.shared_mut_clone
                            .results
                            .lock()
                            .unwrap()
                            .add_count(&seq_match_result.sample_barcode, barcode_string);
                        self.sequence_errors_clone.correct_match()
                    } else {
                        let added = self.shared_mut_clone.results.lock().unwrap().add_random(
                            &seq_match_result.sample_barcode,
                            &seq_match_result.random_barcode,
                            barcode_string,
                        );
                        if added {
                            self.sequence_errors_clone.correct_match()
                        } else {
                            self.sequence_errors_clone.duplicated();
                        }
                    }
                }
            } else {
                if self.shared_mut_clone.finished.load(Ordering::Relaxed) {
                    break;
                }
            }
        }
        Ok(())
    }

    fn get_seqeunce(&mut self) -> bool {
        // Pop off the last sequence from the seq vec
        if let Some(new_sequence) = self.shared_mut_clone.seq.lock().unwrap().pop() {
            self.raw_sequence = RawSequence::new(new_sequence);
            true
        } else {
            false
        }
    }
    fn get_sample_seqs(&mut self) {
        // Get a vec of all possible sample barcodes for error correction
        if let Some(ref samples) = self.samples_clone {
            for sample_barcode in samples.keys() {
                self.sample_seqs.insert(sample_barcode.to_string());
            }
        }
    }
    fn get_barcode_seqs(&mut self) {
        if let Some(ref barcodes) = self.barcodes_clone {
            self.barcode_seqs = barcodes
                .iter()
                .map(|hash| {
                    hash.keys()
                        .map(|key| key.to_string())
                        .collect::<HashSet<String>>()
                })
                .collect::<Vec<HashSet<String>>>();
        }
    }

    /// Does a regex search and captures the barcodes.  Converts the sample barcode to ID.  Returns a String with commas between Sample_ID and
    /// building block barcodes.  This is used as a key within the results vector, where the value can be used as the count
    fn match_seq(&mut self) -> Result<Option<SequenceMatchResult>, Box<dyn Error>> {
        self.check_and_fix_consant_region()?;

        // if the barcodes are found continue, else return None and record a constant region error
        if let Some(barcodes) = self
            .sequence_format_clone
            .format_regex
            .captures(&self.raw_sequence.sequence)
        {
            let match_results = SequenceMatchResult::new(
                barcodes,
                &self.barcode_groups,
                &self.barcode_seqs,
                self.max_errors_clone.max_barcode_errors(),
                &self.sample_seqs,
                self.max_errors_clone.max_sample_errors(),
            )?;

            if match_results.sample_barcode_error {
                self.sequence_errors_clone.sample_barcode_error();
                return Ok(None);
            }
            if match_results.counted_barcode_error {
                self.sequence_errors_clone.barcode_error();
                return Ok(None);
            }
            Ok(Some(match_results))
        } else {
            // If the constant region was not found, record the error and return None
            self.sequence_errors_clone.constant_region_error();
            Ok(None)
        }
    }
    fn check_and_fix_consant_region(&mut self) -> Result<(), Box<dyn Error>> {
        if !self
            .sequence_format_clone
            .format_regex
            .is_match(&self.raw_sequence.sequence)
        {
            self.raw_sequence.fix_constant_region(
                &self.sequence_format_clone.format_string,
                self.max_errors_clone.max_constant_errors(),
            )?;
        }
        Ok(())
    }
}

struct RawSequence {
    sequence: String,
}

impl RawSequence {
    pub fn new(sequence: String) -> RawSequence {
        RawSequence { sequence }
    }

    pub fn insert_barcodes_constant_region(&mut self, format_string: &str, best_sequence: String) {
        // Start a new string to push to
        let mut fixed_sequence = String::new();
        // Push the correct constant region nucleotides.  If the constant string has an N, push the nucleotides from the original
        // sequence corresponding to the barcodes
        for (old_char, new_char) in best_sequence.chars().zip(format_string.chars()) {
            if new_char == 'N' {
                fixed_sequence.push_str(&old_char.to_string());
            } else {
                fixed_sequence.push_str(&new_char.to_string());
            }
        }
        self.sequence = fixed_sequence
    }

    pub fn fix_constant_region(
        &mut self,
        format_string: &str,
        max_constant_errors: u8,
    ) -> Result<(), Box<dyn Error>> {
        // Find the region of the sequence that best matches the constant region.  This is doen by iterating through the sequence
        // Get the length difference between what was sequenced and the barcode region with constant regions
        // This is to stop the iteration in the next step
        let length_diff = self.sequence.len() - format_string.len();

        // Create a vector of sequences the length of the constant region + barcodes to check for where the best match is located
        let mut possible_seqs = Vec::new();
        for index in 0..length_diff {
            let possible_seq = self
                .sequence
                .chars()
                .skip(index) // skip to where the current index is and take the next amount equal to the length of the constant region + barcodes
                .take(format_string.len())
                .collect::<String>();
            // Add the new sequence to the vector of all possible matches
            possible_seqs.push(possible_seq);
        }
        // Find the closest match within what was sequenced to the constant region
        let best_sequence_option = fix_error(format_string, &possible_seqs, max_constant_errors)?;

        if let Some(best_sequence) = best_sequence_option {
            self.insert_barcodes_constant_region(format_string, best_sequence);
            Ok(())
        } else {
            self.sequence = "".to_string();
            Ok(())
        }
    }
}

pub struct SequenceMatchResult {
    pub sample_barcode: String,
    pub counted_barcodes: Vec<String>,
    pub counted_barcode_error: bool,
    pub sample_barcode_error: bool,
    pub random_barcode: String,
}

impl SequenceMatchResult {
    pub fn new(
        barcodes: Captures,
        barcode_groups: &[String],
        barcode_seqs: &Vec<HashSet<String>>,
        counted_barcode_max_errors: &[u8],
        sample_seqs: &HashSet<String>,
        sample_seqs_max_errors: u8,
    ) -> Result<SequenceMatchResult, Box<dyn Error>> {
        let mut sample_barcode_error = false;
        let sample_barcode;
        if let Some(sample_barcode_match) = barcodes.name("sample") {
            let sample_barcode_str = sample_barcode_match.as_str();
            if sample_seqs.contains(sample_barcode_str) {
                sample_barcode = sample_barcode_str.to_string();
            } else {
                let sample_barcode_fix_option =
                    fix_error_hashset(sample_barcode_str, sample_seqs, sample_seqs_max_errors)?;
                if let Some(fixed_barcode) = sample_barcode_fix_option {
                    sample_barcode = fixed_barcode;
                } else {
                    sample_barcode = String::new();
                    sample_barcode_error = true;
                }
            }
        } else {
            sample_barcode = String::new();
        }
        let mut counted_barcode_error = false;
        let mut counted_barcodes = Vec::new();
        if !sample_barcode_error {
            for (index, barcode_group) in barcode_groups.iter().enumerate() {
                let mut counted_barcode =
                    barcodes.name(barcode_group).unwrap().as_str().to_string();
                if !barcode_seqs.is_empty() {
                    if !barcode_seqs[index].contains(&counted_barcode) {
                        let barcode_seq_fix_option = fix_error_hashset(
                            &counted_barcode,
                            &barcode_seqs[index],
                            counted_barcode_max_errors[index],
                        )?;
                        if let Some(fixed_barcode) = barcode_seq_fix_option {
                            counted_barcode = fixed_barcode;
                        } else {
                            counted_barcode_error = true;
                            break;
                        }
                    }
                }
                counted_barcodes.push(counted_barcode);
            }
        }
        let random_barcode;
        if let Some(random_barcode_match) = barcodes.name("random") {
            random_barcode = random_barcode_match.as_str().to_string()
        } else {
            random_barcode = String::new()
        }
        Ok(SequenceMatchResult {
            sample_barcode,
            counted_barcodes,
            counted_barcode_error,
            sample_barcode_error,
            random_barcode,
        })
    }

    pub fn barcode_string(&self) -> String {
        self.counted_barcodes.join(",")
    }
}

/// Fix an error in a sequence by comparing it to all possible sequences.  If no sequence matches with fewer or equal to the number of mismatches 'None' is returned.
/// 'None' is also returned if two or more sequences are best matches,
///
/// # Example
///
/// ```
/// use barcode::parse_sequences::fix_error;
///
/// let barcode = "AGTAG";
///
/// let possible_barcodes_one_match = vec!["AGCAG".to_string(), "ACAAG".to_string(), "AGCAA".to_string()]; // only the first has a single mismatch
/// let possible_barcodes_two_match = vec!["AGCAG".to_string(), "AGAAG".to_string(), "AGCAA".to_string()]; // first and second have a single mismatch
///
/// let max_mismatches = barcode.chars().count() / 5; // allow up to 20% mismatches
///
/// let fixed_error_one = fix_error(barcode, &possible_barcodes_one_match, max_mismatches).unwrap();
/// let fixed_error_two = fix_error(barcode, &possible_barcodes_two_match, max_mismatches).unwrap();
///
/// assert_eq!(fixed_error_one, Some("AGCAG".to_string()));
/// assert_eq!(fixed_error_two, None);
/// ```
pub fn fix_error(
    mismatch_seq: &str,
    possible_seqs: &[String],
    mismatches: u8,
) -> Result<Option<String>, Box<dyn Error>> {
    let mut best_match = None; // start the best match with None
    let mut best_mismatch_count = mismatches + 1; // Add 1 and start the best.  This allows a match with the same mismatches as required
    let mut keep = true; // An initiated variable to check if there is more than one best match

    // Iterate through possible matches
    for true_seq in possible_seqs {
        // Initiate the number of mismatches for the current iterated possible sequece match
        let mut mismatches = 0;

        // Iterate through the nucleotides of the possible match and the sequence to be fixed finding how many mismatches
        // If the mismatches exceed the current best mismatched, end this early
        for (possible_char, current_char) in true_seq.chars().zip(mismatch_seq.chars()) {
            if possible_char != current_char && current_char != 'N' && possible_char != 'N' {
                mismatches += 1;
            }
            if mismatches > best_mismatch_count {
                break;
            }
        }
        // If there are more than one best match, don't keep
        if mismatches == best_mismatch_count {
            keep = false
        }
        // If this is the best match, keep and reset best mismatches to this value
        if mismatches < best_mismatch_count {
            keep = true;
            best_mismatch_count = mismatches;
            best_match = Some(true_seq.to_string());
        }
    }
    // If there is one best match and it is some, return it.  Otherwise return None
    if keep && best_match.is_some() {
        Ok(best_match)
    } else {
        Ok(None)
    }
}

pub fn fix_error_hashset(
    mismatch_seq: &str,
    possible_seqs: &HashSet<String>,
    mismatches: u8,
) -> Result<Option<String>, Box<dyn Error>> {
    let mut best_match = None; // start the best match with None
    let mut best_mismatch_count = mismatches + 1; // Add 1 and start the best.  This allows a match with the same mismatches as required
    let mut keep = true; // An initiated variable to check if there is more than one best match

    // Iterate through possible matches
    for true_seq in possible_seqs {
        // Initiate the number of mismatches for the current iterated possible sequece match
        let mut mismatches = 0;

        // Iterate through the nucleotides of the possible match and the sequence to be fixed finding how many mismatches
        // If the mismatches exceed the current best mismatched, end this early
        for (possible_char, current_char) in true_seq.chars().zip(mismatch_seq.chars()) {
            if possible_char != current_char && current_char != 'N' && possible_char != 'N' {
                mismatches += 1;
            }
            if mismatches > best_mismatch_count {
                break;
            }
        }
        // If there are more than one best match, don't keep
        if mismatches == best_mismatch_count {
            keep = false
        }
        // If this is the best match, keep and reset best mismatches to this value
        if mismatches < best_mismatch_count {
            keep = true;
            best_mismatch_count = mismatches;
            best_match = Some(true_seq.to_string());
        }
    }
    // If there is one best match and it is some, return it.  Otherwise return None
    if keep && best_match.is_some() {
        Ok(best_match)
    } else {
        Ok(None)
    }
}
