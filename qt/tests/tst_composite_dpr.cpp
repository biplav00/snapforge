// Probe for the compositeImage double-DPR bug. Replicates exactly what
// AnnotationCanvas::compositeImage() does: take a backdrop QImage that carries
// a devicePixelRatio (as the cropped screenshot does, tagged in setRegion),
// then paint annotation geometry through `p.scale(dpr, dpr)`. A logical-coord
// mark must land at physical = logical*dpr. If the painter ALSO honours the
// image's devicePixelRatio, the mark lands at logical*dpr*dpr (the bug).

#include <QtTest/QtTest>
#include <QImage>
#include <QPainter>

class TestCompositeDpr : public QObject {
    Q_OBJECT
private:
    // First physical column containing a black pixel (mark left edge).
    static int firstBlackX(const QImage &img) {
        for (int x = 0; x < img.width(); ++x)
            for (int y = 0; y < img.height(); ++y)
                if (qGray(img.pixel(x, y)) < 128) return x;
        return -1;
    }

public:
    // Mirror of compositeImage(): backdrop tagged with dpr, painter scaled by dpr.
    static QImage composite(double dpr, bool fixDpr) {
        const int logicalW = 100, logicalH = 100;
        QImage backdrop(int(logicalW * dpr), int(logicalH * dpr),
                        QImage::Format_ARGB32_Premultiplied);
        backdrop.setDevicePixelRatio(dpr);              // as setRegion() tags it
        backdrop.fill(Qt::white);
        QImage result = backdrop.convertToFormat(QImage::Format_ARGB32_Premultiplied);
        if (fixDpr) result.setDevicePixelRatio(1.0);    // the proposed fix
        QPainter p(&result);
        if (dpr != 1.0) p.scale(dpr, dpr);
        // Annotation at logical x=10 (a 4pt-wide black square).
        p.fillRect(QRectF(10, 10, 4, 4), Qt::black);
        p.end();
        return result;
    }

private slots:
    // The contract compositeImage() now upholds: reset the backdrop's dpr to
    // 1.0, then the single manual p.scale(dpr) maps logical x=10 -> physical 20.
    void contract_resetDpr_then_singleScale() {
        QCOMPARE(firstBlackX(composite(2.0, /*fixDpr=*/true)), 20);
    }

    // Documents the bug the reset prevents: leaving the backdrop's dpr=2 makes
    // QPainter scale on top of p.scale(dpr) -> logical x=10 lands at 40 (the
    // "annotation moved after copy" symptom on Retina). The reset is therefore
    // load-bearing.
    void withoutReset_doubleScales() {
        QCOMPARE(firstBlackX(composite(2.0, /*fixDpr=*/false)), 40);
    }

    // dpr=1 is unaffected either way.
    void dpr1_unaffected() {
        QCOMPARE(firstBlackX(composite(1.0, false)), 10);
    }
};

QTEST_MAIN(TestCompositeDpr)
#include "tst_composite_dpr.moc"
