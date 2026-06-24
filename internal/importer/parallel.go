package importer

import (
	"context"
	"fmt"
	"sync"
)

func parallelFiles(ctx context.Context, paths []string, workers int, fn func(context.Context, string) error) error {
	if workers <= 1 || len(paths) <= 1 {
		for _, p := range paths {
			if err := fn(ctx, p); err != nil {
				return err
			}
		}
		return nil
	}
	if workers > len(paths) {
		workers = len(paths)
	}
	sem := make(chan struct{}, workers)
	errCh := make(chan error, len(paths))
	var wg sync.WaitGroup
	for _, path := range paths {
		wg.Add(1)
		go func(p string) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()
			if err := fn(ctx, p); err != nil {
				errCh <- err
			}
		}(path)
	}
	wg.Wait()
	close(errCh)
	for err := range errCh {
		if err != nil {
			return err
		}
	}
	return nil
}

func fileWorkerErr(table, path string, err error) error {
	return fmt.Errorf("%s %s: %w", table, path, err)
}
