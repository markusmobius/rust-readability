pub(crate) fn sort_by<T>(data: &mut [T], compare: impl Fn(&T, &T) -> bool) {
    let length = data.len();
    let limit = (usize::BITS - length.leading_zeros()) as usize;
    Sorter { data, compare }.partition_sort(0, length, limit);
}

struct Sorter<'a, T, Compare> {
    data: &'a mut [T],
    compare: Compare,
}

impl<T, Compare: Fn(&T, &T) -> bool> Sorter<'_, T, Compare> {
    fn less(&self, first: usize, second: usize) -> bool {
        (self.compare)(&self.data[first], &self.data[second])
    }

    fn insertion(&mut self, first: usize, end: usize) {
        for index in first + 1..end {
            let mut current = index;
            while current > first && self.less(current, current - 1) {
                self.data.swap(current, current - 1);
                current -= 1;
            }
        }
    }

    fn sift_down(&mut self, mut root: usize, end: usize, first: usize) {
        loop {
            let mut child = 2 * root + 1;
            if child >= end {
                break;
            }
            if child + 1 < end && self.less(first + child, first + child + 1) {
                child += 1;
            }
            if !self.less(first + root, first + child) {
                return;
            }
            self.data.swap(first + root, first + child);
            root = child;
        }
    }

    fn heap(&mut self, first: usize, end: usize) {
        let length = end - first;
        for index in (0..=(length - 1) / 2).rev() {
            self.sift_down(index, length, first);
        }
        for index in (0..length).rev() {
            self.data.swap(first, first + index);
            self.sift_down(0, index, first);
        }
    }

    fn partition_sort(&mut self, mut first: usize, mut end: usize, mut limit: usize) {
        let mut balanced = true;
        let mut partitioned = true;
        loop {
            let length = end - first;
            if length <= 12 {
                self.insertion(first, end);
                return;
            }
            if limit == 0 {
                self.heap(first, end);
                return;
            }
            if !balanced {
                self.break_patterns(first, end);
                limit -= 1;
            }
            let (mut pivot, mut hint) = self.choose_pivot(first, end);
            if hint == -1 {
                self.data[first..end].reverse();
                pivot = end - 1 - (pivot - first);
                hint = 1;
            }
            if balanced && partitioned && hint == 1 && self.partial_insertion(first, end) {
                return;
            }
            if first > 0 && !self.less(first - 1, pivot) {
                first = self.partition_equal(first, end, pivot);
                continue;
            }
            let (middle, already_partitioned) = self.partition(first, end, pivot);
            partitioned = already_partitioned;
            let left = middle - first;
            let right = end - middle;
            if left < right {
                balanced = left >= length / 8;
                self.partition_sort(first, middle, limit);
                first = middle + 1;
            } else {
                balanced = right >= length / 8;
                self.partition_sort(middle + 1, end, limit);
                end = middle;
            }
        }
    }

    fn partition(&mut self, first: usize, end: usize, pivot: usize) -> (usize, bool) {
        self.data.swap(first, pivot);
        let mut left = first + 1;
        let mut right = end - 1;
        while left <= right && self.less(left, first) {
            left += 1;
        }
        while left <= right && !self.less(right, first) {
            right -= 1;
        }
        if left > right {
            self.data.swap(right, first);
            return (right, true);
        }
        self.data.swap(left, right);
        left += 1;
        right -= 1;
        loop {
            while left <= right && self.less(left, first) {
                left += 1;
            }
            while left <= right && !self.less(right, first) {
                right -= 1;
            }
            if left > right {
                break;
            }
            self.data.swap(left, right);
            left += 1;
            right -= 1;
        }
        self.data.swap(right, first);
        (right, false)
    }

    fn partition_equal(&mut self, first: usize, end: usize, pivot: usize) -> usize {
        self.data.swap(first, pivot);
        let mut left = first + 1;
        let mut right = end - 1;
        loop {
            while left <= right && !self.less(first, left) {
                left += 1;
            }
            while left <= right && self.less(first, right) {
                right -= 1;
            }
            if left > right {
                break;
            }
            self.data.swap(left, right);
            left += 1;
            right -= 1;
        }
        left
    }

    fn partial_insertion(&mut self, first: usize, end: usize) -> bool {
        let mut index = first + 1;
        for _ in 0..5 {
            while index < end && !self.less(index, index - 1) {
                index += 1;
            }
            if index == end {
                return true;
            }
            if end - first < 50 {
                return false;
            }
            self.data.swap(index, index - 1);
            if index - first >= 2 {
                for previous in (1..index).rev() {
                    if !self.less(previous, previous - 1) {
                        break;
                    }
                    self.data.swap(previous, previous - 1);
                }
            }
            if end - index >= 2 {
                for next in index + 1..end {
                    if !self.less(next, next - 1) {
                        break;
                    }
                    self.data.swap(next, next - 1);
                }
            }
        }
        false
    }

    fn break_patterns(&mut self, first: usize, end: usize) {
        let length = end - first;
        if length < 8 {
            return;
        }
        let mut random = length as u64;
        let modulus = 1usize << (usize::BITS - length.leading_zeros());
        for index in first + (length / 4) * 2 - 1..=first + (length / 4) * 2 + 1 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let mut other = random as usize & (modulus - 1);
            if other >= length {
                other -= length;
            }
            self.data.swap(index, first + other);
        }
    }

    fn ordered(&self, first: usize, second: usize, swaps: &mut usize) -> (usize, usize) {
        if self.less(second, first) {
            *swaps += 1;
            (second, first)
        } else {
            (first, second)
        }
    }

    fn median(&self, first: usize, second: usize, third: usize, swaps: &mut usize) -> usize {
        let (first, second) = self.ordered(first, second, swaps);
        let (second, _) = self.ordered(second, third, swaps);
        self.ordered(first, second, swaps).1
    }

    fn choose_pivot(&self, first: usize, end: usize) -> (usize, i8) {
        let length = end - first;
        let mut swaps = 0;
        let mut left = first + length / 4;
        let mut middle = first + (length / 4) * 2;
        let mut right = first + (length / 4) * 3;
        if length >= 8 {
            if length >= 50 {
                left = self.median(left - 1, left, left + 1, &mut swaps);
                middle = self.median(middle - 1, middle, middle + 1, &mut swaps);
                right = self.median(right - 1, right, right + 1, &mut swaps);
            }
            middle = self.median(left, middle, right, &mut swaps);
        }
        (
            middle,
            match swaps {
                0 => 1,
                12 => -1,
                _ => 0,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::sort_by;

    #[test]
    fn go_sort_patterns() {
        for length in [0, 1, 7, 12, 13, 49, 50, 99, 257, 1024] {
            for mode in 0..6 {
                let mut data: Vec<_> = (0..length)
                    .map(|index| match mode {
                        0 => (index, index),
                        1 => (length - index, index),
                        2 => (0, index),
                        3 => ((index * 37 + index / 7) % 11, index),
                        4 => (index.min(length - index), index),
                        _ => (index % 3, index),
                    })
                    .collect();
                sort_by(&mut data, |first, second| first.0 < second.0);
                assert!(data.windows(2).all(|pair| pair[0].0 <= pair[1].0));
                let mut indices = data.iter().map(|pair| pair.1).collect::<Vec<_>>();
                indices.sort();
                assert_eq!(indices, (0..length).collect::<Vec<_>>());
                if mode == 2 {
                    assert_eq!(data.iter().map(|pair| pair.1).collect::<Vec<_>>(), indices);
                }
            }
        }
    }
}
