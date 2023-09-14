pub fn quorum_size(size: usize) -> usize {
    // N       1   2   3   4   5   6   7   8   9
    // quorum  0   1   1   2   3   3   4   5   5
    // q +1    1   2   2   3   4   4   5   6   6
    if size == 1 {
        return 0;
    }
    if size == 2 || size == 3 {
        return 1;
    }
    let fault_number = (size - 1) / 3;
    let mut quorum_size = size;
    if size == 3 * fault_number + 1 {
        quorum_size = 2 * fault_number;
    } else if size == 3 * fault_number + 2 {
        quorum_size = 2 * fault_number + 1;
    } else if size == 3 * fault_number + 3 {
        quorum_size = 2 * fault_number + 1;
    }
    return quorum_size;
}
