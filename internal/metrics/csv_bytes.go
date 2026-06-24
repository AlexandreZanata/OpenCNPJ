package metrics

// CSVRecordBytes estimates raw CSV bytes read for one semicolon-separated record.
func CSVRecordBytes(fields []string) uint64 {
	if len(fields) == 0 {
		return 0
	}
	var n uint64
	for i, field := range fields {
		if i > 0 {
			n++ // field separator
		}
		n += uint64(len(field))
		n += 2 // opening and closing quote in RFB files
	}
	n++ // line terminator
	return n
}
