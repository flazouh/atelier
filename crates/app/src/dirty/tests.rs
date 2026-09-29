use super::*;

#[test]
fn each_file_counts_once_and_a_rename_is_one() {
    assert_eq!(count(""), 0);
    assert_eq!(count(" M a.txt\0?? new file.txt\0"), 2);
    assert_eq!(count("R  new.txt\0old.txt\0 M b.txt\0"), 2, "the old name is not a file");
}
